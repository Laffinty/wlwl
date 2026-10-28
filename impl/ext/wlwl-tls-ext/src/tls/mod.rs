//! `tls` 命名空间（需求 A-1 / A-3 / A-5）。
//!
//! | 成员 | 形状 |
//! |---|---|
//! | `LISTEN(host, port, opts)` | `INTEGER`（监听句柄） |
//! | `ACCEPT(listener, timeout_ms)` | `INTEGER`（连接句柄）或 `NULL`（超时） |
//! | `CONNECT(host, port, opts)` | `INTEGER`（连接句柄） |
//! | `SEND(handle, bytes)` | `INTEGER`（写出的字节数） |
//! | `RECV(handle, max_bytes, timeout_ms)` | `{"bytes": …, "text": …, "eof": …}` |
//! | `CLOSE(handle)` | `NULL` |
//! | `PEER(handle)` | `{"host": …, "port": …, "certificates": [ … ]}` |
//! | `LOCAL_ADDR(handle)` | `{"host": …, "port": …}` |
//! | `CERT_FINGERPRINT(path)` | `STRING`（**SPKI** SHA-256，小写 hex） |
//! | `CERT_INFO(path)` | `DICT` |
//!
//! # ⚠ **阻塞语义**（ABI v1 的硬限制）
//!
//! 扩展 ABI v1 **没有就绪事件通道**（宿主的 `await_ready` 恒返回 `-1`）
//! ⇒ 本模块的握手、accept、读、写**都在调用线程上阻塞**，与 std 的
//! `HTTP_*` / `SLEEP` 同族 ⇒ **等待期间整个运行时是冻结的**。
//!
//! ⇒ 要「等待期间别的任务照常运行」，需要 **ABI v1.1**（另立）：那条版本
//! 要给 `wlwl_ext_ctx_t` 加一条就绪事件通道，**不是**本 crate 能自己解决的。
//!
//! # ⚠ 安全默认值（需求 A-5）
//!
//! 1. **证书校验默认开** —— `CONNECT` 不给 `verify` ⇒ 校验；
//! 2. **TLS 版本下限默认 1.2** —— `min_version` 缺省 `"1.2"`；
//! 3. **没有「一键关掉全部校验」的开关** —— 要关必须**显式**传
//!    `{"verify": false}`，而那会**同时**打一条告警（走宿主的 `report`
//!    通道 ⇒ 打到 stderr）；
//! 4. **私钥不出现在任何错误消息 / 告警 / 返回值里** —— 私钥的失败消息是
//!    **固定文本**（只有路径），不含任何密钥材料；
//! 5. **明文客户端连 TLS 端口 ⇒ 被拒且给出明确错误** —— **绝不**静默降级
//!    成明文（见 [`accept`]）。

pub mod opts;
pub mod registry;

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::sign::CertifiedKey;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, ServerConfig, ServerConnection, SignatureScheme,
    StreamOwned,
};
use serde_json::Value as Json;

use crate::args::{bytes_arg, int_arg, opts_arg, str_arg};
use crate::bytes::{bytes_to_json, unhex};
use crate::envelope::{dict, Ctx, ExtError, MemberResult, KIND_TLS};
use crate::x509::{self, CertInfo};

use opts::{ConnectOpts, ListenOpts, SniSource};
use registry::{Conn, Entry, Listener, PeerInfo};

fn err(op: &'static str, reason: impl Into<String>) -> ExtError {
    ExtError::new(KIND_TLS, op, reason)
}

// ── 底层流 ────────────────────────────────────────────────────────────────

/// 一个「能读能写」的流（擦掉具体的 TLS 连接类型）。
trait AnyStream: Read + Write + Send {}
impl<T: Read + Write + Send + ?Sized> AnyStream for T {}

/// 一条 TLS 连接（字节流 + 一个用来调超时的 socket 句柄）。
///
/// ⚠ 超时是通过 **socket 本身**（`SO_RCVTIMEO`）设的 ⇒ 用的是 `TcpStream`
/// 的**克隆句柄**（socket 选项是 socket 的属性，不是 fd 包装器的属性）。
struct TlsStream {
    inner: Box<dyn AnyStream>,
    sock: TcpStream,
}

impl Read for TlsStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }
}

impl Write for TlsStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl TlsStream {
    /// 设读超时（作用在底下的 socket 上）。
    fn set_read_timeout_impl(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }
}

impl registry::Stream for TlsStream {
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.set_read_timeout_impl(timeout)
    }
    fn shutdown(&self) {
        let _ = self.sock.shutdown(std::net::Shutdown::Both);
    }
}

/// 记录**读到的第一个字节**的流。
///
/// ⚠ 用途：判断对端是不是**明文客户端**（TLS 记录类型只有 20..=23；
/// 明文 HTTP 请求的首字节是 'G' = 0x47 ⇒ 一眼可辨）。
struct PeekStream {
    inner: TcpStream,
    first: Option<u8>,
}

impl PeekStream {
    fn new(inner: TcpStream) -> PeekStream {
        PeekStream { inner, first: None }
    }
    fn first_byte(&self) -> Option<u8> {
        self.first
    }
}

impl Read for PeekStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        if n > 0 && self.first.is_none() {
            self.first = Some(buf[0]);
        }
        Ok(n)
    }
}

impl Write for PeekStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

// ── 证书 / 私钥 ───────────────────────────────────────────────────────────

/// 读一个 PEM 文件里的**全部**证书。
fn load_cert_chain(path: &Path) -> Result<Vec<CertificateDer<'static>>, String> {
    let iter = CertificateDer::pem_file_iter(path).map_err(|e| {
        format!("cannot read the certificate file '{}': {e}", path.display())
    })?;
    let chain: Vec<CertificateDer<'static>> = iter
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("cannot parse the certificate file '{}': {e}", path.display()))?;
    if chain.is_empty() {
        return Err(format!(
            "the certificate file '{}' contains no CERTIFICATE block",
            path.display()
        ));
    }
    Ok(chain)
}

/// 读一个 PEM 文件里的**第一张**证书。
fn load_first_certificate(path: &Path) -> Result<CertificateDer<'static>, String> {
    let mut iter = CertificateDer::pem_file_iter(path).map_err(|e| {
        format!("cannot read the certificate file '{}': {e}", path.display())
    })?;
    match iter.next() {
        Some(Ok(cert)) => Ok(cert),
        Some(Err(e)) => Err(format!(
            "cannot parse the certificate file '{}': {e}",
            path.display()
        )),
        None => Err(format!(
            "the certificate file '{}' contains no CERTIFICATE block",
            path.display()
        )),
    }
}

/// 读一个 PEM 文件里的私钥。
///
/// ⚠⚠ **私钥绝不出现在错误消息里** —— 所以这里的失败消息是**固定文本**
/// （只有路径），**不**把底层错误原文带出来（底层错误有可能带密钥材料）。
fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>, String> {
    PrivateKeyDer::from_pem_file(path).map_err(|_| private_key_rejected(path))
}

/// 私钥失败的**固定**消息（不含任何密钥材料）。
fn private_key_rejected(path: &Path) -> String {
    format!(
        "the private key file '{}' could not be loaded (it must contain a PEM-encoded \
         PKCS#8, PKCS#1 or SEC1 private key)",
        path.display()
    )
}

// ── 服务器配置 ────────────────────────────────────────────────────────────

/// 按 SNI 主机名选证书的解析器。
#[derive(Debug)]
struct SniResolver {
    default: Arc<CertifiedKey>,
    by_name: BTreeMap<String, Arc<CertifiedKey>>,
}

impl rustls::server::ResolvesServerCert for SniResolver {
    fn resolve(
        &self,
        client_hello: rustls::server::ClientHello<'_>,
    ) -> Option<Arc<CertifiedKey>> {
        client_hello
            .server_name()
            .and_then(|name| self.by_name.get(name).cloned())
            .or_else(|| Some(self.default.clone()))
    }
}

/// 造 `LISTEN` 的服务器配置。
fn build_server_config(o: &ListenOpts) -> Result<Arc<ServerConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let chain = load_cert_chain(&o.cert)?;
    let key = load_private_key(&o.key)?;
    let default_key = Arc::new(
        CertifiedKey::from_der(chain, key, &provider)
            .map_err(|_| private_key_rejected(&o.key))?,
    );

    let mut by_name: BTreeMap<String, Arc<CertifiedKey>> = BTreeMap::new();
    for (host, source) in &o.sni {
        let certified = match source {
            SniSource::File(path) => {
                let chain = load_cert_chain(path)?;
                // 这个文件里可能也带着私钥 ⇒ 有就用它，没有就退回默认的。
                let certified = match load_private_key(path) {
                    Ok(key) => {
                        CertifiedKey::from_der(chain, key, &provider).map_err(|_| {
                            private_key_rejected(path)
                        })?
                    }
                    Err(_) => CertifiedKey::new(chain, default_key.key.clone()),
                };
                Arc::new(certified)
            }
            SniSource::Pair { cert, key: key_path } => {
                let chain = load_cert_chain(cert)?;
                let key = load_private_key(key_path)?;
                let certified = CertifiedKey::from_der(chain, key, &provider)
                    .map_err(|_| private_key_rejected(key_path))?;
                Arc::new(certified)
            }
        };
        by_name.insert(host.clone(), certified);
    }

    let resolver: Arc<dyn rustls::server::ResolvesServerCert> = Arc::new(SniResolver {
        default: default_key,
        by_name,
    });

    let builder = rustls::ServerConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(o.min_version.versions())
        .map_err(|e| format!("the TLS configuration is invalid: {e}"))?;

    // 给了 `ca` ⇒ **要求**客户端证书（用它签发）。
    let mut config = match &o.ca {
        Some(ca_path) => {
            let mut roots = RootCertStore::empty();
            for cert in load_cert_chain(ca_path)? {
                roots.add(cert).map_err(|e| {
                    format!(
                        "the client CA file '{}' is not usable: {e}",
                        ca_path.display()
                    )
                })?;
            }
            let verifier =
                rustls::server::WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider)
                    .build()
                    .map_err(|e| format!("the client CA file is not usable: {e}"))?;
            builder
                .with_client_cert_verifier(verifier)
                .with_cert_resolver(resolver)
        }
        None => builder.with_no_client_auth().with_cert_resolver(resolver),
    };

    if !o.alpn.is_empty() {
        config.alpn_protocols = o.alpn.clone();
    }
    Ok(Arc::new(config))
}

// ── 客户端配置 ────────────────────────────────────────────────────────────

/// **关掉**服务器证书校验的验证器。
///
/// ⚠ 只有在调用方**显式**传了 `{"verify": false}` 时才会被用上 ⇒ 那一路
/// 会**同时**打一条告警（见 [`connect`]）。
#[derive(Debug)]
struct NoServerVerification;

impl ServerCertVerifier for NoServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// 造 `CONNECT` 的客户端配置。
fn build_client_config(o: &ConnectOpts) -> Result<Arc<ClientConfig>, String> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(o.min_version.versions())
        .map_err(|e| format!("the TLS configuration is invalid: {e}"))?;

    let builder = if o.verify {
        // ⚠ **默认走这一条**：校验服务器证书。
        let mut roots = RootCertStore::empty();
        match &o.ca {
            Some(ca_path) => {
                for cert in load_cert_chain(ca_path)? {
                    roots.add(cert).map_err(|e| {
                        format!("the CA file '{}' is not usable: {e}", ca_path.display())
                    })?;
                }
            }
            None => {
                // ⚠ 没给 `ca` ⇒ 用 **Mozilla 根库**（`webpki-roots`）——
                // **不是**空信任库（需求 A-5）。
                roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            }
        }
        let verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
            Arc::new(roots),
            provider,
        )
        .build()
        .map_err(|e| format!("the trust roots are not usable: {e}"))?;
        builder
            .dangerous()
            .with_custom_certificate_verifier(verifier)
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoServerVerification))
    };

    let mut config = match (&o.client_cert, &o.client_key) {
        (Some(cert_path), Some(key_path)) => {
            let chain = load_cert_chain(cert_path)?;
            let key = load_private_key(key_path)?;
            builder
                .with_client_auth_cert(chain, key)
                .map_err(|e| format!("the client certificate is not usable: {e}"))?
        }
        _ => builder.with_no_client_auth(),
    };

    if !o.alpn.is_empty() {
        config.alpn_protocols = o.alpn.clone();
    }
    Ok(Arc::new(config))
}

// ── 握手 ──────────────────────────────────────────────────────────────────

/// 一次握手失败（原始的 `io::Error` + 对端读到的第一个字节）。
struct HandshakeFailure {
    error: io::Error,
    first_byte: Option<u8>,
}

/// 握手连接需要的那点东西。
///
/// ⚠ `complete_io` 是**泛型**方法（rustls 的 `T: Read + Write` 是 `Sized`）
/// ⇒ 这条 trait **不能**做成 `dyn` ⇒ 握手函数也必须是泛型的。
trait Handshake {
    fn is_handshaking(&self) -> bool;
    fn complete_io<T: Read + Write>(
        &mut self,
        io: &mut T,
    ) -> Result<(usize, usize), io::Error>;
}

impl Handshake for ClientConnection {
    fn is_handshaking(&self) -> bool {
        <ClientConnection as std::ops::Deref>::deref(self).is_handshaking()
    }
    fn complete_io<T: Read + Write>(
        &mut self,
        io: &mut T,
    ) -> Result<(usize, usize), io::Error> {
        <ClientConnection as std::ops::DerefMut>::deref_mut(self).complete_io(io)
    }
}

impl Handshake for ServerConnection {
    fn is_handshaking(&self) -> bool {
        <ServerConnection as std::ops::Deref>::deref(self).is_handshaking()
    }
    fn complete_io<T: Read + Write>(
        &mut self,
        io: &mut T,
    ) -> Result<(usize, usize), io::Error> {
        <ServerConnection as std::ops::DerefMut>::deref_mut(self).complete_io(io)
    }
}

/// 握手循环（**阻塞**）。
///
/// `timeout_ms` 是**这次握手**的预算。
fn handshake<C: Handshake, T: Read + Write>(
    conn: &mut C,
    sock: &mut T,
    timeout_ms: u64,
) -> Result<(), HandshakeFailure> {
    let budget = timeout_ms.max(1);
    let deadline = Instant::now() + Duration::from_millis(budget);
    loop {
        if !conn.is_handshaking() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(HandshakeFailure {
                error: io::Error::new(io::ErrorKind::TimedOut, "handshake timeout"),
                first_byte: None,
            });
        }
        match conn.complete_io(sock) {
            Ok(_) => {}
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut {
                    return Err(HandshakeFailure {
                        error: io::Error::new(
                            io::ErrorKind::TimedOut,
                            format!("the TLS handshake did not finish within {budget} ms"),
                        ),
                        first_byte: None,
                    });
                }
                return Err(HandshakeFailure {
                    error: e,
                    first_byte: None,
                });
            }
        }
    }
}

/// 把一次握手失败翻译成**人话**。
///
/// ⚠ **明文客户端**要给出**明确**的错误（需求 A-5 第 5 条）—— 绝不静默
/// 降级成明文。
fn classify_handshake_error(failure: &HandshakeFailure, timeout_ms: u64) -> String {
    // ① 第一字节不是合法的 TLS 记录类型 ⇒ **明文客户端**。
    if let Some(byte) = failure.first_byte {
        if !(20..=23).contains(&byte) {
            return format!(
                "the peer did not start a TLS handshake (its first byte was 0x{byte:02x}, \
                 which is not a TLS record type) — it looks like a plaintext client; the \
                 connection was rejected and closed"
            );
        }
    }
    // ② 超时。
    if failure.error.kind() == io::ErrorKind::TimedOut {
        return format!("the TLS handshake did not finish within {timeout_ms} ms");
    }
    // ③ rustls 自己的错误。
    if let Some(inner) = failure
        .error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
    {
        return match inner {
            rustls::Error::InvalidMessage(_) | rustls::Error::PeerMisbehaved(_) => format!(
                "the peer did not complete a valid TLS handshake (it looks like a plaintext \
                 client) — the connection was rejected and closed: {inner}"
            ),
            rustls::Error::AlertReceived(alert) => {
                format!("the peer rejected the TLS handshake: {alert:?}")
            }
            _ => format!("the TLS handshake failed: {inner}"),
        };
    }
    // ④ 连接被对端掐断。
    if failure.error.kind() == io::ErrorKind::UnexpectedEof {
        return "the peer closed the connection before the TLS handshake finished".to_string();
    }
    format!("the TLS handshake failed: {}", failure.error)
}

// ── 成员 ──────────────────────────────────────────────────────────────────

/// `LISTEN(host, port, opts) -> INTEGER`
pub fn listen(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "LISTEN";
    let host = str_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let port = int_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    if !(0..=65535).contains(&port) {
        return Err(err(OP, format!("the port {port} is out of range 0..=65535")));
    }
    let raw_opts = opts_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;
    let parsed = opts::parse_listen(&raw_opts, OP).map_err(|e| err(OP, e))?;

    let config = build_server_config(&parsed).map_err(|e| err(OP, e))?;

    // `port == 0` ⇒ **临时端口**（由系统挑一个空闲的）。
    let addr = format!("{host}:{port}");
    let resolved: SocketAddr = addr
        .to_socket_addrs()
        .map_err(|e| err(OP, format!("cannot resolve '{addr}': {e}")))?
        .next()
        .ok_or_else(|| err(OP, format!("'{addr}' did not resolve to any address")))?;

    let listener = TcpListener::bind(resolved)
        .map_err(|e| err(OP, format!("cannot listen on '{addr}': {e}")))?;
    let local = listener
        .local_addr()
        .map_err(|e| err(OP, format!("cannot read the local address: {e}")))?;

    let handle = registry::insert(Entry::Listener(Arc::new(Listener {
        listener,
        config,
        local,
    })));
    Ok(Json::from(handle))
}

/// `ACCEPT(listener, timeout_ms) -> INTEGER | NULL`
///
/// - 有一条连接 ⇒ 连接句柄（**握手已完成**）；
/// - 超时 ⇒ `NULL`（**正常值**，与 `TCP_ACCEPT` 同形，不是失败）。
pub fn accept(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "ACCEPT";
    let listener_id = int_arg(args, 0, OP, 2).map_err(|e| err(OP, e))?;
    let timeout_ms = int_arg(args, 1, OP, 2).map_err(|e| err(OP, e))?;
    if timeout_ms < 0 {
        return Err(err(OP, "the timeout must not be negative"));
    }
    let timeout_ms = timeout_ms as u64;

    let listener = match registry::get(listener_id as u32) {
        Some(Entry::Listener(l)) => l,
        Some(Entry::Conn(_)) => {
            return Err(err(
                OP,
                format!("the handle {listener_id} is a connection, not a listener"),
            ))
        }
        None => {
            return Err(err(
                OP,
                format!("there is no listener with the handle {listener_id}"),
            ))
        }
    };

    let Some((sock, peer_addr)) =
        accept_with_deadline(&listener.listener, timeout_ms).map_err(|e| err(OP, e))?
    else {
        // 超时 ⇒ **正常值**。
        return Ok(Json::Null);
    };

    let timeout = Duration::from_millis(timeout_ms.max(1));
    let _ = sock.set_read_timeout(Some(timeout));
    let _ = sock.set_write_timeout(Some(timeout));

    let control = sock
        .try_clone()
        .map_err(|e| err(OP, format!("cannot duplicate the socket handle: {e}")))?;
    let mut peek = PeekStream::new(sock);

    let mut conn = ServerConnection::new(listener.config.clone())
        .map_err(|e| err(OP, format!("cannot start the TLS server connection: {e}")))?;

    // 握手（**阻塞**）。
    let handshake_result = handshake(&mut conn, &mut peek, timeout_ms);
    if let Err(mut failure) = handshake_result {
        failure.first_byte = peek.first_byte();
        // ⚠ 明文客户端 ⇒ 明确错误；连接直接丢掉（**不**降级成明文）。
        return Err(err(OP, classify_handshake_error(&failure, timeout_ms)));
    }

    let peer_certs = conn
        .peer_certificates()
        .map(|certs| {
            certs
                .iter()
                .map(|c| summarize_certificate(c.as_ref()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let owned = StreamOwned::new(conn, peek);
    let stream = TlsStream {
        inner: Box::new(owned),
        sock: control,
    };

    let handle = registry::insert(Entry::Conn(Arc::new(Conn {
        stream: std::sync::Mutex::new(Box::new(stream)),
        peer: PeerInfo {
            host: peer_addr.ip().to_string(),
            port: peer_addr.port(),
            certificates: peer_certs,
        },
    })));
    Ok(Json::from(handle))
}

/// 带截止时间的 accept（`timeout_ms == 0` ⇒ **无限等**）。
fn accept_with_deadline(
    listener: &TcpListener,
    timeout_ms: u64,
) -> Result<Option<(TcpStream, SocketAddr)>, String> {
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("cannot switch the listener to non-blocking mode: {e}"))?;
    let deadline = if timeout_ms == 0 {
        None
    } else {
        Some(Instant::now() + Duration::from_millis(timeout_ms))
    };
    loop {
        match listener.accept() {
            Ok((sock, addr)) => {
                let _ = listener.set_nonblocking(false);
                let _ = sock.set_nonblocking(false);
                return Ok(Some((sock, addr)));
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if let Some(deadline) = deadline {
                    if Instant::now() >= deadline {
                        let _ = listener.set_nonblocking(false);
                        return Ok(None);
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(e) => {
                let _ = listener.set_nonblocking(false);
                return Err(format!("cannot accept a connection: {e}"));
            }
        }
    }
}

/// `CONNECT(host, port, opts) -> INTEGER`
pub fn connect(ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "CONNECT";
    let host = str_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let port = int_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    if !(1..=65535).contains(&port) {
        return Err(err(
            OP,
            format!("the port {port} is out of range 1..=65535"),
        ));
    }
    let raw_opts = opts_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;
    let parsed = opts::parse_connect(&raw_opts, OP).map_err(|e| err(OP, e))?;

    // ⚠ 显式关掉校验 ⇒ **打一条告警**（宿主把它打到 stderr）。
    if !parsed.verify {
        ctx.report(
            "TlsWarning",
            "CONNECT: certificate verification was DISABLED by the caller \
             ({\"verify\": false}) — the server identity is NOT checked",
        );
    }

    let config = build_client_config(&parsed).map_err(|e| err(OP, e))?;

    let server_name_text = parsed.server_name.clone().unwrap_or_else(|| host.clone());
    let server_name = ServerName::try_from(server_name_text.clone())
        .map_err(|_| err(OP, format!("'{server_name_text}' is not a valid server name")))?;

    let addr = format!("{host}:{port}");
    let resolved: SocketAddr = addr
        .to_socket_addrs()
        .map_err(|e| err(OP, format!("cannot resolve '{addr}': {e}")))?
        .next()
        .ok_or_else(|| err(OP, format!("'{addr}' did not resolve to any address")))?;

    let timeout = Duration::from_millis(parsed.connect_timeout_ms.max(1));
    let sock = TcpStream::connect_timeout(&resolved, timeout)
        .map_err(|e| err(OP, format!("cannot connect to '{addr}': {e}")))?;
    let _ = sock.set_read_timeout(Some(timeout));
    let _ = sock.set_write_timeout(Some(timeout));
    let peer_addr = sock
        .peer_addr()
        .map_err(|e| err(OP, format!("cannot read the peer address: {e}")))?;
    let control = sock
        .try_clone()
        .map_err(|e| err(OP, format!("cannot duplicate the socket handle: {e}")))?;

    let mut conn = ClientConnection::new(config, server_name)
        .map_err(|e| err(OP, format!("cannot start the TLS client connection: {e}")))?;

    let mut plain = sock;
    let handshake_result = handshake(&mut conn, &mut plain, parsed.connect_timeout_ms);
    if let Err(failure) = handshake_result {
        return Err(err(
            OP,
            classify_handshake_error(&failure, parsed.connect_timeout_ms),
        ));
    }

    // ── 公钥固定（需求 A-3）────────────────────────────────────────────
    let peer_certs: Vec<CertInfo> = conn
        .peer_certificates()
        .map(|certs| {
            certs
                .iter()
                .map(|c| summarize_certificate(c.as_ref()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if let Some(pin) = &parsed.pin {
        let expected = unhex(pin).map_err(|e| err(OP, e))?;
        let Some(actual) = peer_certs.first().map(|c| c.spki_sha256.clone()) else {
            return Err(err(
                OP,
                "the server did not present a certificate, so the pin could not be checked",
            ));
        };
        let actual_bytes =
            unhex(&actual).map_err(|e| err(OP, format!("internal pin error: {e}")))?;
        if actual_bytes != expected {
            return Err(err(
                OP,
                format!(
                    "the server's public key does not match the pin (expected SPKI \
                     SHA-256 {pin}, got {actual})"
                ),
            ));
        }
    }

    let owned = StreamOwned::new(conn, plain);
    let stream = TlsStream {
        inner: Box::new(owned),
        sock: control,
    };

    let handle = registry::insert(Entry::Conn(Arc::new(Conn {
        stream: std::sync::Mutex::new(Box::new(stream)),
        peer: PeerInfo {
            host: peer_addr.ip().to_string(),
            port: peer_addr.port(),
            certificates: peer_certs,
        },
    })));
    Ok(Json::from(handle))
}

/// `SEND(handle, bytes) -> INTEGER`
pub fn send(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "SEND";
    let handle = int_arg(args, 0, OP, 2).map_err(|e| err(OP, e))?;
    let payload = bytes_arg(args, 1, OP, 2).map_err(|e| err(OP, e))?;
    let conn = connection(handle, OP)?;
    let mut guard = conn.stream.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .write_all(&payload)
        .map_err(|e| err(OP, format!("cannot write to the connection: {e}")))?;
    guard
        .flush()
        .map_err(|e| err(OP, format!("cannot flush the connection: {e}")))?;
    Ok(Json::from(payload.len() as i64))
}

/// `RECV(handle, max_bytes, timeout_ms) -> {"bytes": …, "text": …, "eof": …}`
///
/// - 超时 ⇒ `{"bytes": [], "text": "", "eof": false}`（**不是** EOF）；
/// - 对端关闭 ⇒ `{"bytes": [], "text": "", "eof": true}`。
pub fn recv(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "RECV";
    let handle = int_arg(args, 0, OP, 3).map_err(|e| err(OP, e))?;
    let max_bytes = int_arg(args, 1, OP, 3).map_err(|e| err(OP, e))?;
    let timeout_ms = int_arg(args, 2, OP, 3).map_err(|e| err(OP, e))?;
    if max_bytes < 0 {
        return Err(err(OP, "max_bytes must not be negative"));
    }
    if timeout_ms < 0 {
        return Err(err(OP, "the timeout must not be negative"));
    }
    let conn = connection(handle, OP)?;
    let mut guard = conn.stream.lock().unwrap_or_else(|e| e.into_inner());
    let timeout = if timeout_ms == 0 {
        None
    } else {
        Some(Duration::from_millis(timeout_ms as u64))
    };
    guard
        .set_read_timeout(timeout)
        .map_err(|e| err(OP, format!("cannot set the read timeout: {e}")))?;

    let mut buffer = vec![0u8; max_bytes as usize];
    let n = match guard.read(&mut buffer) {
        Ok(n) => n,
        Err(e)
            if e.kind() == io::ErrorKind::WouldBlock
                || e.kind() == io::ErrorKind::TimedOut =>
        {
            // 超时 ⇒ **正常值**（没数据、也不是 EOF）。
            return Ok(dict(vec![
                ("bytes", bytes_to_json(&[])),
                ("text", Json::String(String::new())),
                ("eof", Json::Bool(false)),
            ]));
        }
        Err(e) => return Err(err(OP, format!("cannot read from the connection: {e}"))),
    };
    buffer.truncate(n);
    let text = String::from_utf8_lossy(&buffer).into_owned();
    Ok(dict(vec![
        ("bytes", bytes_to_json(&buffer)),
        ("text", Json::String(text)),
        ("eof", Json::Bool(n == 0)),
    ]))
}

/// `CLOSE(handle) -> NULL`
pub fn close(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "CLOSE";
    let handle = int_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    match registry::remove(handle as u32) {
        Some(Entry::Conn(conn)) => {
            // 尽量把 FIN 发出去（失败无所谓 —— 关就关了）。
            let guard = conn.stream.lock().unwrap_or_else(|e| e.into_inner());
            guard.shutdown();
            Ok(Json::Null)
        }
        Some(Entry::Listener(_)) => Ok(Json::Null),
        None => Err(err(OP, format!("there is no handle {handle} to close"))),
    }
}

/// `PEER(handle) -> {"host": …, "port": …, "certificates": [ … ]}`
pub fn peer(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "PEER";
    let handle = int_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    let conn = connection(handle, OP)?;
    let certificates: Vec<Json> = conn.peer.certificates.iter().map(cert_to_json).collect();
    Ok(dict(vec![
        ("host", Json::String(conn.peer.host.clone())),
        ("port", Json::from(conn.peer.port as i64)),
        ("certificates", Json::Array(certificates)),
    ]))
}

/// `LOCAL_ADDR(handle) -> {"host": …, "port": …}`
///
/// ⚠ 这一条是 `LISTEN` 的**必要配套**：需求要求 `port = 0` 时用**临时端口**
/// —— 而临时端口**只能**从这里问出来（`LISTEN` 的返回值是句柄，不是端口）。
pub fn local_addr(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "LOCAL_ADDR";
    let handle = int_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    match registry::get(handle as u32) {
        Some(Entry::Listener(listener)) => Ok(dict(vec![
            ("host", Json::String(listener.local.ip().to_string())),
            ("port", Json::from(listener.local.port() as i64)),
        ])),
        Some(Entry::Conn(conn)) => Ok(dict(vec![
            ("host", Json::String(conn.peer.host.clone())),
            ("port", Json::from(conn.peer.port as i64)),
        ])),
        None => Err(err(OP, format!("there is no handle {handle}"))),
    }
}

/// `CERT_FINGERPRINT(path) -> STRING`
///
/// **SPKI**（`subjectPublicKeyInfo`）的 SHA-256，小写 hex ⇒ 公钥固定用。
pub fn cert_fingerprint(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "CERT_FINGERPRINT";
    let path = str_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    let cert = load_first_certificate(Path::new(&path)).map_err(|e| err(OP, e))?;
    let info = x509::parse_certificate(cert.as_ref()).map_err(|e| err(OP, e))?;
    Ok(Json::String(info.spki_sha256))
}

/// `CERT_INFO(path) -> DICT`
pub fn cert_info(_ctx: &Ctx, args: &[Json]) -> MemberResult {
    const OP: &str = "CERT_INFO";
    let path = str_arg(args, 0, OP, 1).map_err(|e| err(OP, e))?;
    let cert = load_first_certificate(Path::new(&path)).map_err(|e| err(OP, e))?;
    let info = x509::parse_certificate(cert.as_ref()).map_err(|e| err(OP, e))?;
    Ok(cert_to_json(&info))
}

// ── 工具 ──────────────────────────────────────────────────────────────────

/// 取一个连接句柄。
fn connection(handle: i64, op: &'static str) -> Result<Arc<Conn>, ExtError> {
    match registry::get(handle as u32) {
        Some(Entry::Conn(c)) => Ok(c),
        Some(Entry::Listener(_)) => Err(err(
            op,
            format!("the handle {handle} is a listener, not a connection"),
        )),
        None => Err(err(op, format!("there is no connection with the handle {handle}"))),
    }
}

/// 一张 DER 证书 ⇒ 要的字段（解析不出来 ⇒ 空字段，但不致命）。
fn summarize_certificate(der: &[u8]) -> CertInfo {
    x509::parse_certificate(der).unwrap_or_else(|_| CertInfo {
        subject: String::new(),
        issuer: String::new(),
        san: Vec::new(),
        not_before: String::new(),
        not_after: String::new(),
        spki_sha256: String::new(),
        serial: String::new(),
    })
}

/// `CertInfo` ⇒ JSON 字典。
fn cert_to_json(info: &CertInfo) -> Json {
    dict(vec![
        ("subject", Json::String(info.subject.clone())),
        ("issuer", Json::String(info.issuer.clone())),
        (
            "san",
            Json::Array(info.san.iter().map(|s| Json::String(s.clone())).collect()),
        ),
        ("not_before", Json::String(info.not_before.clone())),
        ("not_after", Json::String(info.not_after.clone())),
        ("spki_sha256", Json::String(info.spki_sha256.clone())),
        ("serial", Json::String(info.serial.clone())),
    ])
}

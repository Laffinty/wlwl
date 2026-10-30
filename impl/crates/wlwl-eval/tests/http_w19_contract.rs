//! [W-19] `std.http::STATIC_FILE` 的契约 —— 静态文件服务（C-7 / C-10 / C-12）。
//!
//! ## 这张契约表守的是什么
//!
//! `STATIC_FILE(root, path, opts)` 是**静态文件服务**的入口：给一个文档根 +
//! 一个请求路径 ⇒ 产出一个**响应字典**（`status` / `headers` / `body`）。
//! 它把三件事合在**一个**成员里（它们共享同一份「目标文件」的事实）：
//!
//! * **C-7** 静态文件服务原语：路径穿越防护 · MIME · `Content-Length` ·
//!   `ETag` / `Last-Modified` / **304** · `Range` / **206** · `Cache-Control` ·
//!   目录 ⇒ `index.html`。
//! * **C-10** 压缩协商：`Accept-Encoding` ⇒ `Content-Encoding`。
//! * **C-12** 条件请求：`If-None-Match` / `If-Modified-Since` ⇒ **304**。
//!
//! ## ⚠ 为什么本文件**不直接** import 仓库里的 `wlwl:std.http`
//!
//! 本波 `http.wll` 归 W-18（同一文件不能并行写）⇒ `STATIC_FILE` 的 R1 代码
//! 先落在一份**独立文件**里（`SECTION` 常量）。
//!
//! ⇒ 本文件走 `--std-src` 那条**覆盖轨**（`Evaluator::with_std_src`）：
//! 把仓库里的 `wl/std/*.wll` 复制到一个临时目录，再把 `http.wll` 换成
//! 「仓库那份 + 新 IMPORT + `SECTION` + 改了成员名的 `EXPORT`」⇒ **测的就是
//! 即将接进 `http.wll` 的那份代码**，而**不动**仓库里的文件。
//!
//! ⚠ **接进 `http.wll` 之后**，本文件**自动**改用仓库里那份（判定：仓库那份
//! 的 `EXPORT` 里已经有 `STATIC_FILE`）⇒ 覆盖轨退化成「照抄」，测的仍然是
//! 真正生效的那份代码。
//!
//! ## ⚠ 路径穿越防护的**反向守卫**
//!
//! 防护**必须**走 `std.fs::PATH_CONTAINS`（**逐段**前缀判定）——
//! 字符串前缀比较会让**同前缀的兄弟目录** `root-evil` 冒充 `root` 的子路径。
//! ⇒ 把 `SECTION` 里的 `PATH_CONTAINS(r, target0)` 换成字符串前缀比较，
//! `the_traversal_out_of_the_root_is_refused` 那条**必须红**。

mod common;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use wlwl_eval::Evaluator;

/// 仓库里 `wlwl-std` 的 R1 源目录（`wl/std`）。
fn repo_std_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../wlwl-std/wl/std")
}

/// 被测的 R1 段（**还没**接进 `http.wll` 时用它）。
const SECTION: &str = r##"// ═══════════════════════════════════════════════════════════════════════
// ═══════════════════════════════════════════════════════════════════════
// [W-19] `std.http::STATIC_FILE` —— 静态文件服务(C-7 / C-10 / C-12)
// ═══════════════════════════════════════════════════════════════════════
//
// 【这份文件是什么】
//
// 它是**要插进 `impl/crates/wlwl-std/wl/std/http.wll` 的 R1 代码**。
// 本波 `http.wll` 归 W-18（同一文件不能并行写）⇒ 代码先落在**仓库外**的
// 这份文件里，等 W-18 落地后再接进去。
//
// 【接线说明（三步）】
//
// ① **IMPORT** —— **能追加就追加**（同一个名字被两个 `IMPORT` 绑 ⇒ `E0021`）：
//
//    · `wlwl:std.fs` —— 在**现有那一行**里**追加**四个名字：
//      `"PATH_CONTAINS"`、`"PATH_EXT"`、`"FILE_META"`、`"READ_BYTES"`。
//      ⚠ `PATH_JOIN` **已经在那一行里**（W-18 加的）⇒ **不要**再导一次。
//      追加后那一行形如：
//        IMPORT("wlwl:std.fs", ["WRITE_FILE", "MKDIRS", "PATH_JOIN", "PATH_CONTAINS", "PATH_EXT", "FILE_META", "READ_BYTES"]);
//    · `wlwl:std.encode` —— 在**现有那一行**里**追加** `"URL_DECODE"`：
//        IMPORT("wlwl:std.encode", ["BYTE_LEN", "URL_DECODE"]);
//    · `wlwl:std.compress` —— `http.wll` **还没有**导入它 ⇒ **新增一行**：
//        IMPORT("wlwl:std.compress", ["GZIP_COMPRESS", "ZSTD_COMPRESS"]);
//
//    ⚠ 三条都**不要**另起第二行 `IMPORT("同一个模块", …)` ⇒ 那是同一个模块的
//    第二次导入 ⇒ 成员名会重复绑定 ⇒ `E0021`。
//
// ② **本文件的正文**（下面 `// ── 扩展名 ⇒ …` 那一大段）插在
//    `LET(SERVE_STREAM, _SERVE_STREAM);` 这一行**之后**
//    （`EXPORT` 那一行**之前**）。
//
// ③ **EXPORT 行**改成（**仍必须单行** —— `lang_exports` 是逐行扫的）：
//
//        EXPORT(["PARSE_REQUEST", "BUILD_RESPONSE", "SERVE_STREAM", "ROUTE", "MATCH_ROUTE", "PARSE_COOKIES", "SET_COOKIE", "PARSE_FORM", "PARSE_MULTIPART", "STATIC_FILE"]);
//
//    （现有那一行的九个成员**一个不动**，只在**末尾**追加 `"STATIC_FILE"`。）
//
// 【成员语义】
//
// `STATIC_FILE(root: STRING, path: STRING, opts: DICT | NULL) -> DICT`
//
// · `root` = 文档根目录。
// · `path` = **请求目标**（可以带查询串 ⇒ 本成员自己剥掉）。
// · `opts` = 可选策略字典：
//   · `"headers"` = **请求头**（`DICT`，键**小写** —— 与 `PARSE_REQUEST` 同一形状）。
//     `Range` / `Accept-Encoding` / `If-None-Match` / `If-Modified-Since` 都从这里读。
//   · `"compress"` = `BOOLEAN`，缺省 `FALSE` —— **压缩是显式开启的策略**
//     （压缩会改变响应字节，且对已经压缩过的载荷是负收益 ⇒ 不替调用方决定）。
//   · `"cache_control"` = `STRING`，缺省 `"no-cache"`。
//   · `"index"` = `STRING`，缺省 `"index.html"`（目录 ⇒ 这个文件）。
//   · `"method"` = `STRING`，缺省 `"GET"`（`"HEAD"` ⇒ 只发头不发体）。
//
// 返回（**正好**是 `SERVE_STREAM` 的 handler 契约：`status` / `headers` / `body`）：
//
//   `{"status": INTEGER, "headers": DICT, "body": ARRAY[INTEGER]}`
//
// · `headers` 的键**一律小写**（与 `PARSE_REQUEST` / `BUILD_RESPONSE` 同一口径）。
// · `body` 是**字节数组** ⇒ 二进制文件**不会**被有损 UTF-8 化。
//
// 【失败口径（两档，不新增诊断码）】
//
// · **元数 / 类型错** ⇒ 既有 `E0022` / `E0030`（终止运行）。
// · **运行期 IO 失败**（stat / read / compress 失败）⇒ `ERR` **值**
//   （`ERR([kind:"HttpError", op:"STATIC_FILE", reason])`）。
// · 「文件不在」「路径越界」「区间不满足」**不是失败** ⇒ 那是 **404 / 403 / 416**。
//
// 【状态码】
//
// · `200` —— 完整正文。· `206` —— **单区间**。· `304` —— 条件请求命中。
// · `400` —— 请求路径里的百分号转义是坏的。· `403` —— **路径穿越**。
// · `404` —— 文件不在。· `405` —— 方法不是 `GET` / `HEAD`。· `416` —— 区间不可满足。
//
// 【⚠ 路径穿越防护：一律走 `std.fs::PATH_CONTAINS`】
//
// `PATH_CONTAINS` 是**逐段**前缀判定（不是字符串前缀）⇒ `D:/root-evil` 冒充不了
// `D:/root` 的子路径；且它**不访问文件系统** ⇒ 判定对不存在的路径也成立、
// 跨平台逐字一致。
//
// ⚠⚠ **不要**在本模块里自己写字符串前缀比较：`PATH_NORMALIZE` 保留 `\`、
// `PATH_JOIN` 产出 `/` ⇒ 前缀比对在 Windows 上**必然失败** ⇒ 全站 403。
//
// ⚠ **边界**：`PATH_CONTAINS` 是**词法**判定 ⇒ **不解析符号链接**
// （`root` 底下指向 `root` 之外的符号链接，本成员**仍**判合法）。
// 那条边界是 `W-24` 写死的，本成员**沿用**，不自己造第二套。
//
// ═══════════════════════════════════════════════════════════════════════


// ── 扩展名 ⇒ `Content-Type` ───────────────────────────────────────────────
//
// ⚠ 猜不出来 ⇒ `application/octet-stream`（**不猜**别的）。
// ⚠ `text/*` 与 `application/json` 带 `; charset=utf-8` —— 本仓的文本一律 UTF-8。
LET(_STATIC_MIME, ["html": "text/html; charset=utf-8",
                   "htm": "text/html; charset=utf-8",
                   "css": "text/css; charset=utf-8",
                   "js": "text/javascript; charset=utf-8",
                   "mjs": "text/javascript; charset=utf-8",
                   "json": "application/json",
                   "map": "application/json",
                   "txt": "text/plain; charset=utf-8",
                   "md": "text/markdown; charset=utf-8",
                   "csv": "text/csv; charset=utf-8",
                   "xml": "application/xml",
                   "svg": "image/svg+xml",
                   "png": "image/png",
                   "jpg": "image/jpeg",
                   "jpeg": "image/jpeg",
                   "gif": "image/gif",
                   "webp": "image/webp",
                   "avif": "image/avif",
                   "bmp": "image/bmp",
                   "ico": "image/x-icon",
                   "pdf": "application/pdf",
                   "zip": "application/zip",
                   "gz": "application/gzip",
                   "wasm": "application/wasm",
                   "woff": "font/woff",
                   "woff2": "font/woff2",
                   "ttf": "font/ttf",
                   "otf": "font/otf",
                   "mp3": "audio/mpeg",
                   "mp4": "video/mp4",
                   "webm": "video/webm",
                   "wav": "audio/wav",
                   "ogg": "audio/ogg"]);


// ── 响应字典 ──────────────────────────────────────────────────────────────
//
// ⚠ **正好三个键** —— 与 `SERVE_STREAM` 的 handler 契约逐字一致
//   （`status` / `headers` / `body`，缺一即失败）。
LET(_STATIC_RESP, FUN((status, headers, body),
    ["status": status, "headers": headers, "body": body]));


// ── `FILE_META` ⇒ 一个普通字典（**不**把 `ERR` 绑到局部变量上）────────────
//
// ⚠ `ERR` 一旦被 `LET` 绑定，作用域结束就报 `E0103`（§8.6.1）⇒ 失败在这里
//   **就地**折成 `{"ok": FALSE, "reason": …}`，调用方拿到的永远是普通字典。
LET(_STATIC_META, FUN((target),
    (LET(m, FILE_META(target));
     IF(IS_ERR(m),
        (LET(_, m); ["ok": FALSE, "reason": "cannot stat the target file"]),
        ["ok": TRUE, "meta": m]))));


// ── `q` 值（`"0.5"` / `"1"` / `"0"`）⇒ **千分比**整数 ──────────────────────
//
// ⚠ 非法 ⇒ -1（调用方**不选**这个编码 —— 权重算不出来就不声称它可接受）。
// ⚠ 用千分比是为了让 `q` 能**比大小**而不引入浮点。
LET(_PARSE_Q, FUN((s),
    (LET(t, TRIM(s));
     IF(==(t, ""),
        -1,
        (LET(dot, INDEX_OF(t, "."));
         IF(==(dot, -1),
            (LET(w, _PARSE_DEC(t));
             IF(<(w, 0), -1, *(w, 1000))),
            (LET(w, _PARSE_DEC(SUB(t, 0, dot)));
             LET(f, SUB(t, +(dot, 1), -(LEN(t), +(dot, 1))));
             IF(||(<(w, 0), <(f, 0)),
                -1,
                IF(>(LEN(f), 3),
                   -1,
                   +( *(w, 1000),
                      _PARSE_DEC(+(REPEAT("0", -(3, LEN(f))), f)) ))))))))));


// ── `Accept-Encoding` ⇒ 选一个编码（`""` = 不压缩）─────────────────────────
//
// ⚠ 只认 `gzip` / `zstd`（`std.compress` 的**压缩**侧有这两个）；`x-gzip` 是
//   gzip 的历史别名 ⇒ 归到 gzip。
// ⚠ `q=0` ⇒ **明确不接受**（RFC 9110 §12.5.3）⇒ 不选。
// ⚠ 同 `q` ⇒ **按客户端书写的先后**（RFC 9110 §12.5.3 没规定同 q 的排序 ⇒
//   沿用客户端顺序最不意外）。
LET(_PICK_ENCODING, FUN((hdr),
    (LET MUT(best, "");
     LET MUT(best_q, 0);
     IF(==(TYPE(hdr), "NULL"),
        NULL,
        (FOR(item, SPLIT(hdr, ","),
            (LET(parts, SPLIT(item, ";"));
             LET(tok, LOWER(TRIM(AT(parts, 0, ""))));
             LET(q, 1000);
             FOR(p, SLICE(parts, 1, LEN(parts)),
                 (LET(kv, SPLIT(p, "="));
                  IF(==(LOWER(TRIM(AT(kv, 0, ""))), "q"),
                     SET(q, _PARSE_Q(AT(kv, 1, ""))),
                     NULL)));
             IF(!(||(||(==(tok, "gzip"), ==(tok, "zstd")), ==(tok, "x-gzip"))),
                NULL,
                (IF(>(q, best_q),
                   (SET(best_q, q); SET(best, IF(==(tok, "zstd"), "zstd", "gzip"))),
                   NULL))))));
     IF(>(best_q, 0), best, "")))));


// ── 弱 ETag ⇒ 剥掉 `W/` 前缀（RFC 9110 §13.1.2 的**弱比较**）───────────────
LET(_STATIC_WEAK, FUN((s),
    IF(STARTS_WITH(s, "W/"), SUB(s, 2, -(LEN(s), 2)), s)));


// ── `If-None-Match` ⇒ 命中吗？─────────────────────────────────────────────
//
// ⚠ 用**弱比较** ⇒ 两边的 `W/` 前缀都剥掉再比。
// ⚠ `If-None-Match: *` ⇒ 任意现存表示都算命中。
LET(_MATCH_INM, FUN((inm, etag),
    (LET MUT(hit, FALSE);
     IF(==(TYPE(inm), "NULL"),
        FALSE,
        (LET(v, TRIM(inm));
         IF(==(v, "*"),
            TRUE,
            (FOR(item, SPLIT(v, ","),
                (LET(c, TRIM(item));
                 IF(==(_STATIC_WEAK(c), _STATIC_WEAK(etag)),
                    SET(hit, TRUE),
                    NULL)));
             hit))));
     hit)));


// ── `If-Modified-Since` ⇒ 命中吗？──────────────────────────────────────────
//
// ⚠ 只认 **IMF-fixdate**（与本成员自己发的 `Last-Modified` 同一形状）⇒ 别的
//   形状（RFC 850 / asctime）**比不了** ⇒ 不当作命中 ⇒ 走 200。
//   ⚠ **判据**：长度与 `Last-Modified` 相同（IMF-fixdate 恒 29 字符）**且**
//   第 4、5 个字符是 `", "`（RFC 850 是 `"Sunday, 06-Nov-94 …"` ⇒ 那两格是
//   `"ay,"`）⇒ 两条判据合起来把 RFC 850 挡在外面。
//   ⚠ 挡不住的话**宁可选 200**：把「变了」当成「没变」会把**旧内容**发出去。
// ⚠ 日期比较用**字典序**：同一形状的 IMF-fixdate，字典序 == 时间序。
LET(_MATCH_IMS, FUN((ims, lm),
    (IF(==(TYPE(ims), "NULL"),
       FALSE,
       (LET(v, TRIM(ims));
        (IF(==(lm, ""),
           FALSE,
           (IF(!=(LEN(v), LEN(lm)),
              FALSE,
              (IF(!=(SUB(v, 3, 2), ", "),
                 FALSE,
                 >=(v, lm))))))))))));


// ── `Range` 的**单区间** ⇒ 起止（都含）────────────────────────────────────
//
// ⚠ 返回 `["kind": "none" | "range" | "unsatisfiable", …]`。
// ⚠ **后缀区间**（`bytes=-500`）⇒ 最后 500 字节。
// ⚠ **语法错** ⇒ `none`（**忽略**，不是 416）—— 客户端的语法错不该被当成
//   「资源不存在」。
LET(_RANGE_ONE, FUN((lo, hi, size),
    (LET(s0, _PARSE_DEC(lo));
     LET(e0, _PARSE_DEC(hi));
     IF(==(lo, ""),
        (IF(<(e0, 0),
           RETURN(["kind": "none"]),
           NULL);
         IF(<=(e0, 0),
            RETURN(["kind": "unsatisfiable"]),
            NULL);
         IF(>(e0, size),
            RETURN(["kind": "unsatisfiable"]),
            NULL);
         RETURN(["kind": "range", "start": -(size, e0), "end": -(size, 1)])),
        (IF(<(s0, 0),
           RETURN(["kind": "none"]),
           NULL);
         IF(>=(s0, size),
            RETURN(["kind": "unsatisfiable"]),
            NULL);
         IF(==(hi, ""),
            RETURN(["kind": "range", "start": s0, "end": -(size, 1)]),
            NULL);
         IF(<(e0, 0),
            RETURN(["kind": "none"]),
            NULL);
         IF(<(e0, s0),
            RETURN(["kind": "unsatisfiable"]),
            NULL);
         IF(<(e0, -(size, 1)),
            RETURN(["kind": "range", "start": s0, "end": e0]),
            RETURN(["kind": "range", "start": s0, "end": -(size, 1)])))))));

// ── `Range: bytes=…` ⇒ 单区间 ─────────────────────────────────────────────
//
// ⚠ **只支持单区间**：多区间（`bytes=0-1,5-6`）⇒ **忽略整个 `Range`**
//   （RFC 9110 §14.2 允许服务端忽略；本成员不做多区间拼接）。
LET(_PARSE_RANGE, FUN((hdr, size),
    (LET MUT(out, ["kind": "none"]);
     IF(==(TYPE(hdr), "NULL"),
        NULL,
        (LET(v, TRIM(hdr));
         IF(NOT(STARTS_WITH(LOWER(v), "bytes=")),
            NULL,
            (LET(parts, SPLIT(SUB(v, 6, -(LEN(v), 6)), ","));
             IF(!=(LEN(parts), 1),
                NULL,
                (LET(ab, SPLIT(AT(parts, 0, ""), "-"));
                 IF(!=(LEN(ab), 2),
                    NULL,
                    (SET(out, _RANGE_ONE(TRIM(AT(ab, 0, "")), TRIM(AT(ab, 1, "")), size));
                     NULL))))))));
     out)));


// ── STATIC_FILE(root, path, opts) -> DICT ──────────────────────────────────
LET(STATIC_FILE, FUN((root, path, opts),
    (LET(r, _NEED_STR("STATIC_FILE", root));
     LET(p, _NEED_STR("STATIC_FILE", path));
     LET(o, IF(==(TYPE(opts), "NULL"), DICT(), _NEED_DICT("STATIC_FILE", opts)));
     LET(h0, AT(o, "headers", DICT()));
     LET(h, IF(==(TYPE(h0), "NULL"), DICT(), _NEED_DICT("STATIC_FILE", h0)));
     LET(method, UPPER(_NEED_STR("STATIC_FILE", AT(o, "method", "GET"))));
     LET(compress, AT(o, "compress", FALSE));
     LET(cc, AT(o, "cache_control", "no-cache"));
     LET(index_name, AT(o, "index", "index.html"));
     LET MUT(status, 200);
     LET MUT(body, []);
     LET MUT(extra, DICT());

     // ① 只服务 GET / HEAD（其余 ⇒ 405）
     IF(!(||(==(method, "GET"), ==(method, "HEAD"))),
        RETURN(_STATIC_RESP(405, ["allow": "GET, HEAD", "content-length": "0"], [])),
        NULL);

     // ② 剥查询串 ⇒ 百分号解码（**解码在穿越判定之前** ⇒ `%2e%2e%2f` 也被挡住）
     LET(qi, INDEX_OF(p, "?"));
     LET(raw, IF(==(qi, -1), p, SUB(p, 0, qi)));
     LET(dec, URL_DECODE(raw));
     IF(IS_ERR(dec),
        (LET(_, dec); RETURN(_STATIC_RESP(400, ["content-length": "0"], []))),
        NULL);

     // ③ 相对路径（去掉前导 `/`）⇒ 目标路径
     LET(rel, IF(STARTS_WITH(dec, "/"), SUB(dec, 1, -(LEN(dec), 1)), dec));
     LET(target0, PATH_JOIN(r, rel));

     // ④ ⚠⚠ 路径穿越防护 —— **一律走 `PATH_CONTAINS`**（逐段判定，跨平台一致）。
     //    ⚠ **不要**在这里写字符串前缀比较：`D:/root-evil` 会冒充 `D:/root`
     //    的子路径；而且 `PATH_NORMALIZE` 保留 `\`、`PATH_JOIN` 产出 `/`
     //    ⇒ 前缀比对在 Windows 上**必然失败** ⇒ 全站 403。
     IF(NOT(PATH_CONTAINS(r, target0)),
        RETURN(_STATIC_RESP(403, ["content-length": "0"], [])),
        NULL);

     // ⑤ 元数据（**一次** stat 的快照：`std.fs::FILE_META`）
     LET(m0, _STATIC_META(target0));
     IF(==(AT(m0, "ok", FALSE), FALSE),
        RETURN(_HTTP_ERR("STATIC_FILE", AT(m0, "reason", "cannot stat the target file"))),
        NULL);
     LET(meta0, AT(m0, "meta", DICT()));
     LET(is_dir, ==(AT(meta0, "is_dir", FALSE), TRUE));
     LET(target, IF(is_dir, PATH_JOIN(target0, index_name), target0));
     LET(meta, IF(is_dir,
                   (LET(m1, _STATIC_META(target));
                    IF(==(AT(m1, "ok", FALSE), FALSE),
                       DICT(),
                       AT(m1, "meta", DICT()))),
                   meta0));

     // ⑥ 不存在 ⇒ 404（目录里没有 `index.html` 也是 404）
     IF(NOT(AT(meta, "exists", FALSE)),
        RETURN(_STATIC_RESP(404, ["content-length": "0"], [])),
        NULL);

     // ⑦ `ETag` = `mtime` + `size`（**弱** ETag：`W/"…"`）
     //    ⚠ 弱 ETag 是因为 `mtime + size` **不是**内容摘要 ⇒ 强 ETag 会
     //    声称「逐字节相同」而实际不保证。
     //    ⚠ `Last-Modified` 用 `FILE_META` 给的 IMF-fixdate（UTC）。
     LET(size, AT(meta, "size", 0));
     LET(mtime, AT(meta, "mtime", 0));
     LET(etag, CAT("W/\"", STR(mtime), "-", STR(size), "\""));
     LET(lm, AT(meta, "mtime_http", ""));

     // ⑧ 条件请求命中 ⇒ 304（**无正文、无 `Content-Length`**）
     //    ⚠ RFC 9110 §15.4.5：304 的 `Content-Length` 只在「与 200 的载荷长度
     //    相同」时才允许发 ⇒ 干脆不发。
     IF(||(_MATCH_INM(AT(h, "if-none-match", NULL), etag),
           _MATCH_IMS(AT(h, "if-modified-since", NULL), lm)),
        (LET MUT(h304, DICT());
         SET(h304, INDEX_SET(h304, "etag", etag));
         SET(h304, INDEX_SET(h304, "last-modified", lm));
         SET(h304, INDEX_SET(h304, "cache-control", cc));
         IF(compress,
            SET(h304, INDEX_SET(h304, "vary", "accept-encoding")),
            NULL);
         RETURN(_STATIC_RESP(304, h304, []))),
        NULL);

     // ⑨ `Range`（**单区间**）⇒ 206 / 416
     //    ⚠ 两条**平铺**的 `IF`（不是嵌套的 `if/else`）—— R1 的 `(` 是**块**，
     //    嵌套的 `if/else` 会把括号配平变成一件极容易出错的事。
     LET(rng, _PARSE_RANGE(AT(h, "range", NULL), size));
     IF(==(AT(rng, "kind", "none"), "unsatisfiable"),
        (SET(status, 416);
         SET(extra, ["content-range": CAT("bytes */", STR(size))])),
        NULL);
     IF(==(AT(rng, "kind", "none"), "range"),
        (SET(status, 206);
         SET(extra, ["content-range": CAT("bytes ", STR(AT(rng, "start", 0)), "-", STR(AT(rng, "end", 0)), "/", STR(size))])),
        NULL);

     // ⑩ 读正文（`READ_BYTES` ⇒ **字节数组**，二进制不会被有损 UTF-8 化）
     LET(b, IF(==(status, 416), [], READ_BYTES(target)));
     IF(IS_ERR(b),
        (LET(_, b); RETURN(_HTTP_ERR("STATIC_FILE", "cannot read the target file"))),
        NULL);
     SET(body, b);
     IF(==(status, 206),
        (SET(body, SLICE(b, AT(rng, "start", 0), +(AT(rng, "end", 0), 1)))),
        NULL);

     // ⑪ 压缩协商（`Accept-Encoding` ⇒ `Content-Encoding`）
     //     ⚠ `compress` 缺省是 `FALSE` ⇒ **默认不压缩**。
     LET(enc, IF(compress, _PICK_ENCODING(AT(h, "accept-encoding", NULL)), ""));
     IF(==(enc, ""),
        NULL,
        (LET(c, IF(==(enc, "gzip"), GZIP_COMPRESS(body), ZSTD_COMPRESS(body)));
         IF(IS_ERR(c),
            (LET(_, c); RETURN(_HTTP_ERR("STATIC_FILE", "cannot compress the target file"))),
            (SET(body, c)))));

     // ⑫ 响应头
     LET MUT(hs, DICT());
     SET(hs, INDEX_SET(hs, "content-type", AT(_STATIC_MIME, LOWER(PATH_EXT(target)), "application/octet-stream")));
     SET(hs, INDEX_SET(hs, "content-length", STR(LEN(body))));
     SET(hs, INDEX_SET(hs, "etag", etag));
     SET(hs, INDEX_SET(hs, "last-modified", lm));
     SET(hs, INDEX_SET(hs, "cache-control", cc));
     IF(==(status, 416),
        NULL,
        SET(hs, INDEX_SET(hs, "accept-ranges", "bytes")));
     IF(==(enc, ""),
        NULL,
        SET(hs, INDEX_SET(hs, "content-encoding", enc)));
     IF(compress,
        SET(hs, INDEX_SET(hs, "vary", "accept-encoding")),
        NULL);
     FOR(k, KEYS(extra), SET(hs, INDEX_SET(hs, k, AT(extra, k, ""))));

     // ⑬ `HEAD` ⇒ 只发头不发体（`Content-Length` 仍是 `GET` 会有的那个）
     ["status": status,
      "headers": hs,
      "body": IF(==(method, "HEAD"), [], body)])));
"##;

/// 用例程序的 IMPORT 头。
const IMP: &str = concat!(
    r#"IMPORT("wlwl:std.http", ["STATIC_FILE", "PARSE_REQUEST"]);"#,
    "\n",
    r#"IMPORT("wlwl:std.fs", ["READ_BYTES"]);"#,
    "\n",
    r#"IMPORT("wlwl:std.compress", ["GZIP_DECOMPRESS"]);"#,
    "\n",
    r#"IMPORT("wlwl:std.str", ["CAT"]);"#,
);

/// 一条用例：程序 + 期望的**渲染串**。
struct Case {
    name: &'static str,
    src: &'static str,
    expect: &'static str,
}

/// 夹具的**基目录**（里面放 `root/` 与它的兄弟 `root-evil/`）。
fn fixture_base() -> &'static Path {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    CELL.get_or_init(|| {
        let base = common::scratch("wlwl_http_w19");
        let root = base.join("root");
        std::fs::create_dir_all(root.join("sub")).expect("create the doc root");
        std::fs::write(root.join("index.html"), b"<h1>root</h1>\n").expect("index.html");
        std::fs::write(root.join("a.txt"), b"hello static file\n").expect("a.txt");
        std::fs::write(root.join("style.css"), b"body{color:red}\n").expect("style.css");
        std::fs::write(root.join("data.json"), b"{\"k\":1}\n").expect("data.json");
        std::fs::write(root.join("sub/index.html"), b"<h1>sub</h1>\n").expect("sub/index.html");
        // ⚠ **同前缀的兄弟目录**（`root` vs `root-evil`）——
        //   字符串前缀比较会放行它 ⇒ 这就是反向守卫的靶子。
        std::fs::create_dir_all(base.join("root-evil")).expect("create root-evil");
        std::fs::write(base.join("root-evil/secret.txt"), b"TOP SECRET\n").expect("secret.txt");
        base
    })
}

/// 文档根（**规范路径**：恒 `/` —— `std.fs` 的路径口径）。
fn doc_root() -> String {
    wlwl_path(&fixture_base().join("root"))
}

/// 平台路径 ⇒ 规范路径（恒 `/`）+ wlwl 字符串字面量里的转义。
fn wlwl_path(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").replace('"', "\\\"")
}

/// 覆盖轨的目录：仓库的 `wl/std/*.wll` + 拼好的 `http.wll`。
fn std_src_dir() -> &'static Path {
    static CELL: OnceLock<PathBuf> = OnceLock::new();
    CELL.get_or_init(|| {
        let dir = common::scratch("wlwl_http_w19_src");
        let repo = repo_std_dir();
        for entry in std::fs::read_dir(&repo).expect("read the repo std dir") {
            let p = entry.expect("dir entry").path();
            if p.extension().and_then(|e| e.to_str()) == Some("wll") {
                let name = p.file_name().expect("file name");
                std::fs::copy(&p, dir.join(name)).expect("copy a std source");
            }
        }
        let http = std::fs::read_to_string(dir.join("http.wll")).expect("http.wll");
        // ⚠ **已经接进 `http.wll` 了** ⇒ 直接用仓库那份（覆盖轨退化成照抄）。
        let patched = if wlwl_std::lang_exports(&http)
            .iter()
            .any(|n| n == "STATIC_FILE")
        {
            http
        } else {
            assemble_http(&http)
        };
        std::fs::write(dir.join("http.wll"), patched).expect("write the patched http.wll");
        dir
    })
}

/// 仓库那份 `http.wll` ⇒ 「接好线」的那份。
///
/// ① **IMPORT**：能**追加**就追加（同一个名字被两个 `IMPORT` 绑 ⇒ `E0021`）；
/// ② 本段的正文插在 `EXPORT` 那一行**之前**；
/// ③ `EXPORT` 行**追加** `"STATIC_FILE"`（**仍单行** —— `lang_exports` 逐行扫）。
fn assemble_http(repo: &str) -> String {
    let mut lines: Vec<String> = repo.lines().map(|s| s.to_string()).collect();
    ensure_import(
        &mut lines,
        "wlwl:std.fs",
        &["PATH_CONTAINS", "PATH_EXT", "FILE_META", "READ_BYTES"],
    );
    ensure_import(&mut lines, "wlwl:std.encode", &["URL_DECODE"]);
    ensure_import(
        &mut lines,
        "wlwl:std.compress",
        &["GZIP_COMPRESS", "ZSTD_COMPRESS"],
    );
    let export_idx = lines
        .iter()
        .position(|l| l.trim_start().starts_with("EXPORT(["))
        .expect("http.wll must have an EXPORT line");
    let mut out: Vec<String> = Vec::new();
    out.extend(lines[..export_idx].iter().cloned());
    out.push(SECTION.trim_end().to_string());
    out.push(export_line_with(&lines[export_idx], "STATIC_FILE"));
    out.extend(lines[(export_idx + 1)..].iter().cloned());
    out.join("\n")
}

/// 保证 `module` 的 `names` **在**它那一行里 —— 已有就**追加**，没有就**新增一行**。
fn ensure_import(lines: &mut Vec<String>, module: &str, names: &[&str]) {
    let marker = format!("IMPORT(\"{module}\"");
    if let Some(l) = lines.iter_mut().find(|l| l.contains(&marker)) {
        for n in names {
            let quoted = format!("\"{n}\"");
            if !l.contains(&quoted) {
                let idx = l
                    .rfind(']')
                    .expect("an IMPORT line must close its name list");
                l.insert_str(idx, &format!(", {quoted}"));
            }
        }
        return;
    }
    let list: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    let line = format!("IMPORT(\"{module}\", [{}]);", list.join(", "));
    let last_import = lines
        .iter()
        .rposition(|l| l.trim_start().starts_with("IMPORT("))
        .expect("http.wll must have IMPORT lines");
    lines.insert(last_import + 1, line);
}

/// `EXPORT([...])` 那一行 ⇒ 追加一个成员（**仍单行**）。
fn export_line_with(line: &str, extra: &str) -> String {
    let mut names: Vec<String> = line
        .split('"')
        .skip(1)
        .step_by(2)
        .map(|s| s.to_string())
        .collect();
    if !names.iter().any(|n| n == extra) {
        names.push(extra.to_string());
    }
    let inner: Vec<String> = names.iter().map(|n| format!("\"{n}\"")).collect();
    format!("EXPORT([{}]);", inner.join(", "))
}

/// 跑一段程序 ⇒ 渲染串（失败 ⇒ `!EXXXX 消息`）。
fn actual(src: &str) -> String {
    let ast = wlwl_parser::parse(src, "t.wll")
        .unwrap_or_else(|e| panic!("case source must parse: {src}\n{e}"));
    let mut ev = Evaluator::new()
        .with_base_dir(common::scratch("wlwl_http_w19_case"))
        .with_std_src(std_src_dir().to_path_buf());
    match ev.eval(&ast) {
        Ok(v) => v.display(),
        Err(e) => {
            let d = e.diagnostic();
            let code = d.code.as_str();
            let loc = d.location.clone();
            if d.message.is_empty() {
                format!("!{code} at {loc:?}")
            } else {
                format!("!{code} {} at {loc:?}", d.message)
            }
        }
    }
}

/// 跑一条用例并断言。
fn check(case: &Case) {
    let src = format!("{IMP} {}", case.src).replace("{root}", &doc_root());
    let got = actual(&src);
    assert_eq!(
        case.expect,
        got,
        "case: {name}\n  expected: {want}\n  actual:   {got}",
        name = case.name,
        want = case.expect,
        got = got
    );
}

// ── 契约表 ────────────────────────────────────────────────────────────────

const CASES: &[Case] = &[
    // ── 路径穿越防护（C-7 的核心）─────────────────────────────────────────
    Case {
        name: "the_traversal_out_of_the_root_is_refused",
        src: r#"LET(r, STATIC_FILE("{root}", "/../root-evil/secret.txt", NULL)); CAT(STR(r["status"]), " ", STR(LEN(r["body"])))"#,
        expect: "403 0",
    },
    Case {
        name: "a_traversal_that_looks_like_a_sibling_directory_is_refused",
        // ⚠ `root-evil` 与 `root` **同前缀** ⇒ 字符串前缀比较会放行它。
        src: r#"LET(r, STATIC_FILE("{root}", "/../root-evil/secret.txt", NULL)); STR(r["headers"]["content-length"])"#,
        expect: "0",
    },
    Case {
        name: "an_absolute_looking_request_path_stays_inside_the_root",
        // ⚠ 请求路径**永远**是相对根的 ⇒ `/etc/passwd` 只是根下的 `etc/passwd`。
        src: r#"STR(STATIC_FILE("{root}", "/etc/passwd", NULL)["status"])"#,
        expect: "404",
    },
    // ── 正常文件 · MIME · Content-Length ──────────────────────────────────
    Case {
        name: "a_normal_file_is_served_with_its_content_length",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", NULL)); CAT(STR(r["status"]), " ", r["headers"]["content-length"], " ", r["headers"]["content-type"])"#,
        expect: "200 18 text/plain; charset=utf-8",
    },
    Case {
        name: "the_content_type_of_a_css_file_is_guessed",
        src: r#"STATIC_FILE("{root}", "/style.css", NULL)["headers"]["content-type"]"#,
        expect: "text/css; charset=utf-8",
    },
    Case {
        name: "the_content_type_of_a_json_file_is_guessed",
        src: r#"STATIC_FILE("{root}", "/data.json", NULL)["headers"]["content-type"]"#,
        expect: "application/json",
    },
    Case {
        name: "the_body_is_the_file_content",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", NULL)); STR(r["body"])"#,
        expect:
            "[104, 101, 108, 108, 111, 32, 115, 116, 97, 116, 105, 99, 32, 102, 105, 108, 101, 10]",
    },
    // ── ETag / Last-Modified / 304（C-12）────────────────────────────────
    Case {
        name: "the_etag_is_the_same_for_two_calls_on_the_same_file",
        src: r#"LET(a, STATIC_FILE("{root}", "/a.txt", NULL)); LET(b, STATIC_FILE("{root}", "/a.txt", NULL)); STR(==(a["headers"]["etag"], b["headers"]["etag"]))"#,
        expect: "TRUE",
    },
    Case {
        name: "the_etag_is_a_weak_etag_of_the_mtime_and_the_size",
        src: r#"LET(a, STATIC_FILE("{root}", "/a.txt", NULL)); CAT(STR(STARTS_WITH(a["headers"]["etag"], "W/\"")), " ", STR(ENDS_WITH(a["headers"]["etag"], "-18\"")))"#,
        expect: "TRUE TRUE",
    },
    Case {
        name: "the_last_modified_is_an_imf_fixdate",
        src: r#"LET(a, STATIC_FILE("{root}", "/a.txt", NULL)); CAT(STR(ENDS_WITH(a["headers"]["last-modified"], " GMT")), " ", STR(LEN(a["headers"]["last-modified"])))"#,
        expect: "TRUE 29",
    },
    Case {
        name: "a_matching_if_none_match_gives_304_without_a_body",
        src: r#"LET(a, STATIC_FILE("{root}", "/a.txt", NULL)); LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-none-match": AT(a["headers"], "etag", "")]])); CAT(STR(b["status"]), " ", STR(LEN(b["body"])), " ", AT(b["headers"], "content-length", "none"))"#,
        expect: "304 0 none",
    },
    Case {
        name: "a_non_matching_if_none_match_gives_200",
        src: r#"LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-none-match": "W/\"0-0\""]])); STR(b["status"])"#,
        expect: "200",
    },
    Case {
        name: "a_matching_if_modified_since_gives_304",
        src: r#"LET(a, STATIC_FILE("{root}", "/a.txt", NULL)); LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-modified-since": AT(a["headers"], "last-modified", "")]])); CAT(STR(b["status"]), " ", STR(LEN(b["body"])))"#,
        expect: "304 0",
    },
    Case {
        name: "an_if_modified_since_in_another_date_format_is_ignored",
        // ⚠ RFC 850 形状（`"Sunday, 06-Nov-94 08:49:37 GMT"`）⇒ **比不了** ⇒ 走 200。
        src: r#"LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-modified-since": "Sunday, 06-Nov-94 08:49:37 GMT"]])); STR(b["status"])"#,
        expect: "200",
    },
    Case {
        name: "an_if_modified_since_older_than_the_file_gives_200",
        src: r#"LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-modified-since": "Thu, 01 Jan 1970 00:00:00 GMT"]])); STR(b["status"])"#,
        expect: "200",
    },
    Case {
        // ⚠ **这条是「星期名不参与比较」的守卫**（收口夜实测出的真缺陷）。
        //
        // IMF-fixdate 的头 5 格是 `"Xxx, "`（星期名）⇒ 旧实现拿**整串**比
        // **字典序** ⇒ **星期名排在最前面、会支配整个结果**：实测「文件是
        // `Fri, 30 Oct 2026 …`」时，`Thu` / `Mon` / `Wed` / `Sun, 01 Jan 1970 …`
        // （**1970 年**！）**全部**被判成「没变」⇒ 客户端拿到 304（无正文）
        // ⇒ **缓存住旧内容**。
        //
        // ⇒ 这里**故意用字典序最大的星期名 `Wed`**：无论文件 mtime 落在
        // 星期几，`"Wed" >= 任何星期名` 恒成立 ⇒ 旧实现**恒返回 304**
        // ⇒ 这条守卫**与运行当天是星期几无关**（否则它自己也会变成一条
        // 「按星期漂移」的 flake）。
        name: "an_if_modified_since_whose_weekday_sorts_higher_is_still_compared_by_date",
        src: r#"LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-modified-since": "Wed, 01 Jan 1970 00:00:00 GMT"]])); STR(b["status"])"#,
        expect: "200",
    },
    Case {
        // ⚠ **反向的那一半**：客户端的日期**比文件新** ⇒ 文件自那时起**没变过**
        // ⇒ 条件请求命中 ⇒ **304**（RFC 9110 §13.1.3：「资源的最后修改时间
        // **早于或等于**该日期 ⇒ 条件为假 ⇒ 304」）。
        // ⚠ 旧实现（整串字典序）在**碰巧**的情况下给对 ⇒ 这条不单独当守卫
        // （守卫是上一条）；它钉的是**语义方向**：不能因为「比文件旧 ⇒ 200」
        // 就把「比文件新 ⇒ 304」也一起弄反。
        name: "an_if_modified_since_newer_than_the_file_gives_304",
        src: r#"LET(b, STATIC_FILE("{root}", "/a.txt", ["headers": ["if-modified-since": "Wed, 01 Jan 2038 00:00:00 GMT"]])); STR(b["status"])"#,
        expect: "304",
    },
    // ── Range / 206 ──────────────────────────────────────────────────────
    Case {
        name: "a_range_request_gives_206_with_the_slice",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["headers": ["range": "bytes=0-4"]])); CAT(STR(r["status"]), " ", STR(LEN(r["body"])), " ", r["headers"]["content-range"], " ", STR(r["body"]))"#,
        expect: "206 5 bytes 0-4/18 [104, 101, 108, 108, 111]",
    },
    Case {
        name: "a_suffix_range_gives_the_tail",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["headers": ["range": "bytes=-5"]])); CAT(STR(r["status"]), " ", STR(LEN(r["body"])), " ", STR(r["body"]))"#,
        expect: "206 5 [102, 105, 108, 101, 10]",
    },
    Case {
        name: "an_unsatisfiable_range_gives_416",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["headers": ["range": "bytes=99-120"]])); CAT(STR(r["status"]), " ", r["headers"]["content-range"], " ", STR(LEN(r["body"])))"#,
        expect: "416 bytes */18 0",
    },
    // ── 目录 ⇒ index.html ────────────────────────────────────────────────
    Case {
        name: "a_directory_serves_its_index_html",
        src: r#"LET(r, STATIC_FILE("{root}", "/", NULL)); CAT(STR(r["status"]), " ", r["headers"]["content-type"], " ", STR(LEN(r["body"])))"#,
        expect: "200 text/html; charset=utf-8 14",
    },
    Case {
        name: "a_subdirectory_serves_its_own_index_html",
        src: r#"LET(r, STATIC_FILE("{root}", "/sub/", NULL)); CAT(STR(r["status"]), " ", STR(LEN(r["body"])))"#,
        expect: "200 13",
    },
    // ── 压缩协商（C-10）───────────────────────────────────────────────────
    Case {
        name: "the_accept_encoding_negotiation_picks_gzip",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["compress": TRUE, "headers": ["accept-encoding": "gzip, deflate"]])); CAT(r["headers"]["content-encoding"], " ", STR(==(GZIP_DECOMPRESS(r["body"]), READ_BYTES("{root}/a.txt"))))"#,
        expect: "gzip TRUE",
    },
    Case {
        name: "the_compressed_body_is_not_the_raw_bytes",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["compress": TRUE, "headers": ["accept-encoding": "gzip"]])); STR(!=(LEN(r["body"]), 18))"#,
        expect: "TRUE",
    },
    Case {
        name: "compression_is_off_unless_it_is_asked_for",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["headers": ["accept-encoding": "gzip"]])); CAT(STR(r["status"]), " ", AT(r["headers"], "content-encoding", "none"))"#,
        expect: "200 none",
    },
    Case {
        name: "an_unknown_encoding_leaves_the_body_alone",
        src: r#"LET(r, STATIC_FILE("{root}", "/a.txt", ["compress": TRUE, "headers": ["accept-encoding": "br"]])); CAT(STR(r["status"]), " ", AT(r["headers"], "content-encoding", "none"), " ", STR(LEN(r["body"])))"#,
        expect: "200 none 18",
    },
    // ── 404 / 405 ─────────────────────────────────────────────────────────
    Case {
        name: "a_missing_file_is_404",
        src: r#"STR(STATIC_FILE("{root}", "/nope.txt", NULL)["status"])"#,
        expect: "404",
    },
    Case {
        name: "a_method_other_than_get_or_head_is_405",
        src: r#"STR(STATIC_FILE("{root}", "/a.txt", ["method": "POST"])["status"])"#,
        expect: "405",
    },
    // ── 硬约束的回归守卫（三条）──────────────────────────────────────────
    Case {
        name: "a_chunked_request_body_is_still_refused",
        // ⚠ ADR-0028 D3 —— 本项**不许**放松它。
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
    Case {
        name: "a_duplicate_content_length_is_still_refused",
        // ⚠ **同名多值一律拒**（**即便值相同**）—— 本项**不许**放松它。
        src: r#"LET(r, PARSE_REQUEST("POST /x HTTP/1.1\r\nContent-Length: 5\r\nContent-Length: 5\r\n\r\nhello")); LET(_, r); IF(IS_ERR(r), ERR_PAYLOAD(r)["kind"], "NOT_ERR")"#,
        expect: "HttpError",
    },
];

#[test]
fn the_static_file_contract_holds() {
    for case in CASES {
        check(case);
    }
}

/// ⚠ **导出面**：`STATIC_FILE` 必须**真的**在 `std.http` 的导出面上。
#[test]
fn the_export_surface_carries_static_file() {
    let names = wlwl_std::lang_exports(
        &std::fs::read_to_string(std_src_dir().join("http.wll")).expect("the patched http.wll"),
    );
    assert!(
        names.iter().any(|n| n == "STATIC_FILE"),
        "the export surface of wlwl:std.http must carry STATIC_FILE; got {names:?}"
    );
}

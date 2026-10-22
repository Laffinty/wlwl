// [v1.0 V-04] 并发册附录 H 的小步语义 —— 逐条迁移规则的**可观察取证**。
//
// 一致性册 §6.2 的 **V-04**：「附录 H 的小步语义与参考实现的实际调度行为
// 一致」,验证手段逐字写的是「逐条迁移规则对一条可观测行为」。
//
// 本文件是 V-04 的**其余规则**的取证(H.3.2 / H.3.6 的「切段 / 不切段」
// 两格已由 `concurrency.rs` 的
// `yield_suspension_does_not_replay_the_segment_head` 与
// `blocking_channel_suspension_replays_the_segment_head_exactly_once` 取证)。
//
// 纪律:
//   1. **期望值手写自规范**(并发册 附录 H + §17.0 / §17.1 / §17.2 / §17.3),
//      不是「先跑一遍再抄实现输出」。
//   2. 每条 (A) 类规则都配**负向验证**:把期望改成错的形态 ⇒ 该用例必红。
//      (负向验证读数见交付报告。)
//   3. 被测程序写在**临时目录**,不落进被跟踪的夹具目录 ⇒ 不动 `WLT_FILES`
//      清单,也不脏工作区。
//   4. **不许挂死**:每条用例都有**有界**结束路径 —— 通道要么配对、要么关闭,
//      任务要么到达终态、要么被取消;另有 `RUN_TIMEOUT_SECS` 兜底杀掉子进程。
//      一条挂死的用例会卡住整条 CI 流水线。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// 挂死护栏:被测程序超过这个秒数还没结束 ⇒ 杀掉子进程并让用例失败。
const RUN_TIMEOUT_SECS: u64 = 30;

/// 一次 `wlwl run` 的读数。
struct RunOutput {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn wlwl_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_wlwl") {
        return PathBuf::from(p);
    }
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    for profile in ["debug", "release"] {
        let candidate =
            here.join("target")
                .join(profile)
                .join(if cfg!(windows) { "wlwl.exe" } else { "wlwl" });
        if candidate.exists() {
            return candidate;
        }
    }
    panic!("wlwl binary not found -- run `cargo build -p wlwl-cli --bin wlwl` first");
}

/// 把一段 WLWL 源码写进**临时目录**,返回它的路径。
///
/// 夹具目录(`impl/tests/concurrency`)是**被跟踪**的:测试往里写文件会弄脏
/// 工作区,而且下一次跑会撞上上一次的残留。⇒ 这里用系统临时目录,并且用
/// 自己的子目录名,避免和 `concurrency.rs` 的 scratch 目录互相覆盖。
fn scratch_program(name: &str, source: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("wlwl-scheduling-semantics-scratch");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let p = dir.join(name);
    std::fs::write(&p, source).expect("scratch program written");
    p
}

/// 跑一个 WLWL 程序,带**有界**结束路径。
///
/// 管道在子线程里读(避免「子进程写满管道 ⇒ 父子互等」的经典死锁),
/// 主线程轮询 `try_wait`,超预算就 `kill`。
fn run_wlwl(path: &Path) -> RunOutput {
    let bin = wlwl_binary();
    let mut child = Command::new(&bin)
        .arg("run")
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn `wlwl run {}`: {}", path.display(), e));

    let mut out_pipe = child.stdout.take().expect("stdout piped");
    let mut err_pipe = child.stderr.take().expect("stderr piped");
    let out_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out_pipe.read_to_string(&mut s);
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err_pipe.read_to_string(&mut s);
        s
    });

    let deadline = Instant::now() + Duration::from_secs(RUN_TIMEOUT_SECS);
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!(
                        "`wlwl run {}` exceeded the {RUN_TIMEOUT_SECS}s budget (hang guard)",
                        path.display()
                    );
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => panic!("failed to poll `wlwl run {}`: {e}", path.display()),
        }
    };

    RunOutput {
        code: status.code(),
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    }
}

/// 断言:程序跑通(exit 0)且 stdout **精确等于** `expected`。
#[track_caller]
fn assert_stdout_eq(out: &RunOutput, expected: &str, what: &str) {
    assert_eq!(
        out.code,
        Some(0),
        "{what}: expected exit 0, got {:?}:\n  stderr={}",
        out.code,
        out.stderr
    );
    assert_eq!(
        out.stdout, expected,
        "{what}: stdout mismatch\n  got:      {:?}\n  expected: {:?}",
        out.stdout, expected
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.2 的「保留」行 —— YIELD 切段恢复后,**本段已建立的绑定仍然可见**。
//
// 规范依据(并发册):
//   H.3.2「保留  本段已建立的绑定对恢复后仍然可见(§17.0)」
//   §17.0 的示例(第 94–106 行)给出**金标准输出**:`7 / 1` ——
//   `LET(x, 7); YIELD(); PRINT(x)` 打印 `7`,任务终值 `1`。
//
// 为什么这条不是既有那条(`切段 ⇒ 段首不重跑`)的重复:切段之后段首**不**重跑,
// 于是 `x` 只能靠**环境被保留**才可见 ⇒ 若实现在 YIELD 处丢环境,这里就是
// `E0020`(undefined name)而不是 `7`。两条断言互不覆盖。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_2_yield_resume_keeps_bindings_established_before_the_suspension_point() {
    let src = r#"
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() ,
    LET(x, 7);
    YIELD();
    PRINT(x);
    1
  )));
  LET(v, AWAIT(t));
  PRINT(v)
));
"#;
    let path = scratch_program("h3_2_yield_binding_kept.wll", src);
    let out = run_wlwl(&path);
    // NEG-01
    let expected = "7\n1\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.2 保留:本段已建立的绑定在 YIELD 恢复后仍然可见(§17.0 示例输出 7 / 1)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.2.2 —— 挂起点那一步**不产生值**;挂起点所在的最外层表达式**不完成**。
//
// 规范依据(并发册):
//   H.2.2「结论:该步骤**不**产生值」
//   §17.0「不完成。挂起点所在的最外层表达式**不完成** —— 它**不**产生值」
//   §17.1 的表:`LET(x, YIELD())` → 「整条 LET **不完成**:`x` **根本没绑定**,
//   后续引用 `x` → `E0020: undefined name`」
//
// ⇒ 判据:`LET(x, YIELD())` 之后引用 `x` 必须报 `E0020`。
// 若 YIELD 那一步产生了值(NULL),`x` 就会被绑定 ⇒ 没有 E0020。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h2_2_suspension_point_produces_no_value_so_the_enclosing_let_never_binds() {
    let src = r#"
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() ,
    LET(x, YIELD());
    PRINT(x);
    1
  )));
  LET(v, AWAIT(t));
  PRINT(v)
));
"#;
    let path = scratch_program("h2_2_yield_produces_no_value.wll", src);
    let out = run_wlwl(&path);
    assert_ne!(
        out.code,
        Some(0),
        "H.2.2: 挂起点所在表达式不完成 ⇒ 引用未绑定的 x 必须失败,却跑通了:\n  stdout={}",
        out.stdout
    );
    // NEG-02
    assert!(
        out.stderr.contains("E0020"),
        "H.2.2: expected E0020 (undefined name) for the never-bound `x`, got:\n  stderr={}",
        out.stderr
    );
    assert!(
        out.stderr.contains("undefined name"),
        "H.2.2: expected an `undefined name` diagnostic, got:\n  stderr={}",
        out.stderr
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.9 —— `AWAIT` **不**切段:调用任务的段不中断,段首代码不重跑,
// AWAIT 调用**正常求值完成**。
//
// 规范依据(并发册):
//   H.3.9「τ 的段**不**中断:τ.state 保持 running,τ.cur 不变」
//        「读法  不切段:τ 的段首代码不重跑;AWAIT 调用正常求值完成」
//        「(§17.0 三类挂起点表的第三行)」
//   §17.0 的表第三行:`AWAIT(task)` → 切段「**不切段**」、恢复起点「`AWAIT` 调用处」、
//   挂起点之前的代码「**不重跑**」、挂起点所在表达式「正常求值完成」。
//
// 附录 H 第 621 行的判据提示:「`AWAIT` 这一格的判据与前两类**不同**:前两类看
// 「段首是否重放」,`AWAIT` 看「调用任务是否在 `AWAIT` 处继续求值」」。
// ⇒ 判据:等待方在 AWAIT **之前**的代码只跑一次(没有重放),AWAIT 之后继续执行,
// 并且拿到子任务的终值。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_9_await_does_not_interrupt_the_calling_segment_and_returns_normally() {
    let src = r#"
SCOPE(FUN(() ,
  LET(child, SPAWN(FUN(() , 42)));
  LET(parent, SPAWN(FUN(() ,
    PRINT("await-head");
    LET(v, AWAIT(child));
    PRINT(v);
    PRINT("await-tail")
  )));
  AWAIT(parent)
));
"#;
    let path = scratch_program("h3_9_await_no_segment_interrupt.wll", src);
    let out = run_wlwl(&path);
    // NEG-03
    let expected = "await-head\n42\nawait-tail\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.9:AWAIT 不切段(段首不重跑)且调用正常完成并交付子任务终值",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.5 —— **对端配对**:一侧到达通道操作、对端已停泊 ⇒ 值交给对端,
// 双方重新就绪并继续推进。
//
// 规范依据(并发册):
//   H.3.5「结论  值交给对端;双方状态 := pending;双方进入调度队列尾部」
//        「依据  §17.2.1 对端配对;§17.8 通道配对顺序」
//   §17.2.1「接收方到达且有等待中的发送方 → 取走该发送方的值,双方就绪」
//
// ⇒ 判据:接收方拿到**发送方投的那个值**(不是 NULL、不是 ERR)。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_5_paired_peer_receives_the_value_handed_over() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(r, SPAWN(FUN(() , CHANNEL_RECV(ch))));
  LET(s, SPAWN(FUN(() , CHANNEL_SEND(ch, 42))));
  LET(v, AWAIT(r));
  PRINT(v)
));
"#;
    let path = scratch_program("h3_5_peer_pairing.wll", src);
    let out = run_wlwl(&path);
    // NEG-04
    let expected = "42\n";
    assert_stdout_eq(&out, expected, "H.3.5:对端配对时值交给对端(接收方拿到 42)");
}

// ────────────────────────────────────────────────────────────────────────
// H.3.3 —— **阻塞型 SEND 挂起**:缓冲已满 / 无等待中的接收方 ⇒ 发送方停泊,
// 移出运行队列;对端到达后才恢复。
//
// 规范依据(并发册):
//   H.3.3「前置  K[ch].buf 已满且 K[ch].recvWait 为空」
//        「结论  τ.state := suspended(ch, send);sendWait := … ⧺ [τ];Q := Q \ τ」
//   §17.2.1「`SEND`:缓冲满,且没有等待中的接收方 → 挂起」
//
// ⇒ 判据:发送方在 SEND 处**让出控制**(不是直接跑完)—— 队列里排在它后面的
// 任务必须先跑完,发送方的「SEND 之后」那段代码才出现。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_3_blocking_send_parks_the_sender_until_a_peer_arrives() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(s, SPAWN(FUN(() , CHANNEL_SEND(ch, 42); PRINT("send-done"))));
  LET(x, SPAWN(FUN(() , PRINT("other"))));
  LET(r, SPAWN(FUN(() , LET(v, CHANNEL_RECV(ch)); PRINT(v))));
  AWAIT(r);
  AWAIT(s);
  AWAIT(x)
));
"#;
    let path = scratch_program("h3_3_blocking_send_parks.wll", src);
    let out = run_wlwl(&path);
    // NEG-05
    let expected = "other\nsend-done\n42\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.3:阻塞型 SEND 挂起 ⇒ 发送方让出控制,对端到达后才继续",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.4 —— **阻塞型 RECV 挂起**:缓冲为空且无等待中的发送方 ⇒ 接收方停泊。
//
// 规范依据(并发册):
//   H.3.4「前置  K[ch].buf 为空且 K[ch].sendWait 为空」
//        「结论  τ.state := suspended(ch, recv);recvWait := … ⧺ [τ];Q := Q \ τ」
//   §17.2.1「`RECV`:缓冲空,且没有等待中的发送方 → 挂起」
//
// ⇒ 判据:接收方在 RECV 处**让出控制** —— 排在它后面的任务先跑完,接收方
// 才拿到值。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_4_blocking_recv_parks_the_receiver_until_a_peer_arrives() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(r, SPAWN(FUN(() , LET(v, CHANNEL_RECV(ch)); PRINT(v))));
  LET(x, SPAWN(FUN(() , PRINT("other"))));
  LET(s, SPAWN(FUN(() , CHANNEL_SEND(ch, 42))));
  AWAIT(r);
  AWAIT(s);
  AWAIT(x)
));
"#;
    let path = scratch_program("h3_4_blocking_recv_parks.wll", src);
    let out = run_wlwl(&path);
    // NEG-06
    let expected = "other\n42\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.4:阻塞型 RECV 挂起 ⇒ 接收方让出控制,发送方到达后才拿到值",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.5 的后半句 —— 配对成功的双方进入调度队列**尾部**。
//
// 规范依据(并发册):
//   H.3.5「结论  值交给对端;双方状态 := pending;双方进入调度队列尾部」
//   §17.8「通道配对顺序:对端配对成功的双方都被标记为就绪并加入运行队列**尾部**,
//          避免一对抢占其他就绪任务」
//   §17.4「① 从运行队列按 FIFO 取出一个就绪任务(§17.8)」
//
// ⇒ 判据:配对发生时队列里**已经就绪**的任务仍然排在配对双方**前面** ——
// 一对被唤醒的任务不能插到别的就绪任务前面去。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_5_paired_peers_are_requeued_at_the_tail_behind_already_ready_tasks() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(s, SPAWN(FUN(() , CHANNEL_SEND(ch, 42); PRINT("s-resume"))));
  LET(r, SPAWN(FUN(() , LET(v, CHANNEL_RECV(ch)); PRINT("r-resume"))));
  LET(t, SPAWN(FUN(() , PRINT("t-ready"))));
  AWAIT(r);
  AWAIT(s);
  AWAIT(t)
));
"#;
    let path = scratch_program("h3_5_pair_requeued_at_tail.wll", src);
    let out = run_wlwl(&path);
    // NEG-17
    let expected = "t-ready\ns-resume\nr-resume\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.5:配对双方进入调度队列尾部(已在队的就绪任务先跑)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.3.7 —— **关闭**:`CHANNEL_CLOSE` 置 closed;**停泊的接收方全部释放**,
// 其 RECV 按「关闭且读空」完成,返回 `ERR(kind="ChannelClosed")`。
//
// 规范依据(并发册):
//   H.3.7「结论  K[ch].closed := true
//               停泊的接收方全部释放:其 RECV 按「关闭且读空」完成,
//               返回 ERR(kind="ChannelClosed")」
//        「依据  §17.2.3 关闭协议;§17.7 通道关闭唤醒次序」
//   §17.2.3「关闭**必须**释放该通道上全部挂起的等待方 ——
//              挂起的**接收方**被唤醒后,其 RECV 按「关闭且读空」完成,
//              返回 ERR(kind="ChannelClosed")」
//
// ⇒ 判据:一个**已经停泊**的接收方被关闭唤醒后,拿到的是
// `ERR(kind="ChannelClosed")`。
//
// ⚠ 只覆盖 H.3.7 的**接收方**那一半。发送方那一半(「停泊的发送方全部释放:
// 其 SEND 以 E0054 失败」)在参考实现上**不成立**(并发册 §17.2.3 第 285 行
// 与附录 H 第 695 行的实现注已声明该差异)⇒ 按一致性册 §5 登记为差异,
// 不在本文件里造假断言。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_7_close_releases_a_parked_receiver_with_channelclosed_err() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(r, SPAWN(FUN(() ,
    LET(v, CHANNEL_RECV(ch));
    IF(IS_ERR(v), PRINT(AT_K(ERR_PAYLOAD(v), "kind", "?")), PRINT("not-err"))
  )));
  LET(c, SPAWN(FUN(() , CHANNEL_CLOSE(ch); 0)));
  AWAIT(r)
));
"#;
    let path = scratch_program("h3_7_close_wakes_receiver.wll", src);
    let out = run_wlwl(&path);
    // NEG-07
    let expected = "ChannelClosed\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.7:关闭释放停泊的接收方,其 RECV 返回 ERR(kind=\"ChannelClosed\")",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.4.1 —— `TASK_CANCEL`:置待取消标记;**取消本身不产生用户 ERR 值**。
//
// 规范依据(并发册):
//   H.4.1「结论  C' = C[ 目标任务.cancel := true, 目标任务.reason := reason]」
//        「读法  取消本身不产生用户 ERR 值(§17.3)」
//   §17.3「取消请求携带结构化 `reason` 载荷,**不**产生用户 `ERR` 值;
//          它只在 `AWAIT` 观察点显现」
//
// ⇒ 判据:取消发起方**照常往下跑**(取消点没有 ERR 冒出来),错误只在
// `AWAIT` 那一个观察点显现。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h4_1_task_cancel_itself_raises_no_user_err() {
    let src = r#"
SCOPE(FUN(() ,
  LET(h, SPAWN(FUN(() , 1)));
  TASK_CANCEL(h, ["why": "stop"]);
  PRINT("canceller-alive");
  LET(r, AWAIT(h));
  IF(IS_ERR(r), PRINT("target-cancelled"), PRINT("target-not-cancelled"));
  LET(_, r)
));
"#;
    let path = scratch_program("h4_1_cancel_raises_no_err.wll", src);
    let out = run_wlwl(&path);
    // NEG-08
    let expected = "canceller-alive\ntarget-cancelled\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.4.1:取消本身不产生用户 ERR 值(发起方照常继续,ERR 只在 AWAIT 显现)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.4.5 —— `AWAIT` **观察取消**:已取消的任务的终值是
// `ERR(kind="Cancelled", task: <句柄>, reason: <取消原因>)`。
//
// 规范依据(并发册):
//   H.4.5「前置  任务 τ 的状态为 cancelled」
//        「结论  AWAIT(τ) 返回 ERR(kind="Cancelled", task: <τ 的句柄>,
//               reason: <τ 的取消原因>)」
//        「依据  §17.3 结构化 reason;§17.1 AWAIT」
//   §17.3「`AWAIT` 已取消任务时返回的载荷为
//          ["kind": "Cancelled", "task": <handle>, "reason": <dict>];
//          `reason` 直接透传用户传入的字典,不丢弃、不改写」
//
// ⇒ 判据:载荷的 `kind` 是 "Cancelled";`reason` 原样透传用户给的字典;
// `task` 字段存在。`task` 的**字面形态**(句柄 id/gen 的排印)不钉 ——
// 那是实现细节,规范只要求它承载句柄。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h4_5_await_of_a_cancelled_task_returns_the_cancelled_payload() {
    let src = r#"
SCOPE(FUN(() ,
  LET(h, SPAWN(FUN(() , 1)));
  TASK_CANCEL(h, ["why": "stop"]);
  LET(p, ERR_PAYLOAD(AWAIT(h)));
  PRINT(AT_K(p, "kind", "?"));
  PRINT(AT_K(AT_K(p, "reason", DICT()), "why", "?"));
  PRINT(==(AT_K(p, "task", ""), ""))
));
"#;
    let path = scratch_program("h4_5_await_cancelled_payload.wll", src);
    let out = run_wlwl(&path);
    // NEG-09
    let expected = "Cancelled\nstop\nFALSE\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.4.5:AWAIT 观察取消 ⇒ ERR(kind=Cancelled, task=…, reason=原样透传)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.4.3 —— **检查点观察取消**:取消是协作式的,在下一个检查点可观测;
// 任务在检查点之后终止(检查点之后的代码不再执行)。
//
// 规范依据(并发册):
//   H.4.3「前置  任务 τ 到达检查点 —— YIELD、阻塞型通道操作、
//               TASK_IS_CANCELLED() 三者之一(§17.8);
//               或调度器的取消扫描扫到 τ 的待取消标记(§17.8)」
//        「结论  C' = C[ τ.state := cancelled ];若 τ 停泊在通道 ch 上,清空它的等待登记」
//        「读法  取消是协作式的:不打断正在执行的表达式(§17.3)」
//   §17.8「在飞兄弟取消:目标任务在下一个 `YIELD`、阻塞型通道操作或
//          `TASK_IS_CANCELLED()` 处观察到取消请求」
//
// ⇒ 判据:目标任务跑到第一个 YIELD 检查点(**之前**的代码已经执行 —— 协作式,
// 不打断正在执行的表达式);取消请求发出后,它在**下一个**检查点终止 ⇒
// 检查点之后的那段代码**不执行**。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h4_3_cancel_is_observed_at_the_next_checkpoint_only() {
    let src = r#"
SCOPE(FUN(() ,
  LET(ch, CHANNEL_NEW(0));
  LET(a, SPAWN(FUN(() ,
    PRINT("a1");
    YIELD();
    CHANNEL_RECV(ch);
    PRINT("a2");
    1
  )));
  LET(b, SPAWN(FUN(() , TASK_CANCEL(a); 0)));
  LET(r, AWAIT(a));
  IF(IS_ERR(r), PRINT("a-cancelled"), PRINT("a-not-cancelled"));
  LET(_, r)
));
"#;
    let path = scratch_program("h4_3_cancel_at_next_checkpoint.wll", src);
    let out = run_wlwl(&path);
    // NEG-10
    let expected = "a1\na-cancelled\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.4.3:取消在下一个检查点可观测(检查点前的代码已跑,检查点后的代码不再跑)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.4.2 —— `TASK_CANCEL_PARENT`:当前任务**及其所在作用域子树的全部任务**
// 置待取消标记。
//
// 规范依据(并发册):
//   H.4.2「前置  在任务 τ 内执行」
//        「结论  τ 及其所在作用域子树的全部任务 cancel := true」
//        「依据  §17.3 取消与错误;§17.8 在飞兄弟取消」
//   §17.3「`TASK_CANCEL_PARENT(reason?)` → 请求取消当前任务及当前作用域子树
//          (自顶向下)」
//
// ⇒ 判据:在任务 b 里发起的取消,影响到**同一作用域里的兄弟任务** a
// (a 不是被直接指名的目标任务)⇒ a 也进入 cancelled;并且 b **自身**也被标记
// (`TASK_IS_CANCELLED()` 立刻读得到)。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h4_2_cancel_parent_cancels_the_whole_scope_subtree() {
    let src = r#"
SCOPE(FUN(() ,
  LET(a, SPAWN(FUN(() , PRINT("a1"); YIELD(); PRINT("a2"); 1)));
  LET(b, SPAWN(FUN(() , TASK_CANCEL_PARENT(); PRINT(TASK_IS_CANCELLED()); 0)));
  LET(r, AWAIT(a));
  IF(IS_ERR(r), PRINT("a-cancelled"), PRINT("a-not-cancelled"));
  LET(_, r)
));
"#;
    let path = scratch_program("h4_2_cancel_parent_subtree.wll", src);
    let out = run_wlwl(&path);
    // NEG-11
    let expected = "a1\nTRUE\na-cancelled\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.4.2:TASK_CANCEL_PARENT 取消整个作用域子树(兄弟任务 a 也被取消)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.4.4 —— **SHIELD 推迟**:屏蔽块内的 `TASK_CANCEL_PARENT` 只被记录,
// 不作用于任务;最外层 SHIELD 退出后,取消请求才转为待取消标记。
//
// 规范依据(并发册):
//   H.4.4「前置  屏蔽块内的 TASK_CANCEL_PARENT 被调用,屏蔽层数 > 0」
//        「结论  取消请求只被记录,不被作用于任务;
//               最外层 SHIELD 退出后,取消请求转为 H.4.1 的待取消标记」
//        「依据  §17.3 SHIELD 语义第 1—3 条」
//   §17.3 的 SHIELD 语义第 2、3 条 + 其示例(第 359–374 行)给出**金标准
//   输出**:`FALSE / TRUE / 1` —— 屏蔽块内 `TASK_IS_CANCELLED()` 为 FALSE,
//   最外层 SHIELD 退出后为 TRUE,任务终值 1。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h4_4_shield_defers_the_cancel_until_the_outermost_shield_exits() {
    let src = r#"
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() ,
    SHIELD(FUN(() ,
      TASK_CANCEL_PARENT(["why": "deferred"]);
      PRINT(TASK_IS_CANCELLED());
      0
    ));
    PRINT(TASK_IS_CANCELLED());
    1
  )));
  LET(r, AWAIT(t));
  PRINT(r)
));
"#;
    let path = scratch_program("h4_4_shield_defers_cancel.wll", src);
    let out = run_wlwl(&path);
    // NEG-12
    let expected = "FALSE\nTRUE\n1\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.4.4:SHIELD 只记录取消请求,最外层 SHIELD 退出后才生效(§17.3 示例 1)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.5.1 的**例外** —— 无活动的 SCOPE 时不派生任务,报 `E0058`。
//
// 规范依据(并发册):
//   H.5.1「前置  当前存在活动的 SCOPE」
//        「结论  新任务 τ 登记到当前作用域;τ.state := pending;
//               C' = C[ Q := Q ⧺ [τ] ]」
//        「例外  无活动的 SCOPE 时不派生任务,报 E0058(§17.1 无隐式 runtime scope)」
//   §17.1「**无隐式 runtime scope**:任何 `SPAWN` **必须**直接或间接嵌套在某个
//          `SCOPE(...)` 内;顶层 `SPAWN` → `E0058`」
//
// ⇒ 判据:顶层 SPAWN 报 E0058(而不是悄悄派生一个游离任务)。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h5_1_spawn_without_an_active_scope_is_rejected_with_e0058() {
    let src = r#"
LET(h, SPAWN(FUN(() , 0)));
PRINT(h)
"#;
    let path = scratch_program("h5_1_spawn_without_scope.wll", src);
    let out = run_wlwl(&path);
    assert_ne!(
        out.code,
        Some(0),
        "H.5.1 例外:顶层 SPAWN 必须报 E0058,却跑通了:\n  stdout={}",
        out.stdout
    );
    // NEG-13
    assert!(
        out.stderr.contains("E0058"),
        "H.5.1 例外:expected E0058 (no active SCOPE), got:\n  stderr={}",
        out.stderr
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.5.2 —— **SCOPE 进入**:压入子作用域;`fn` 在**调用者的执行上下文**中
// **同步**求值(它不是被调度的任务)。
//
// 规范依据(并发册):
//   H.5.2「前置  SCOPE(fn) 正在求值」
//        「结论  压入子作用域;fn 在调用者的执行上下文中同步求值(§17.1)」
//   §17.1「`SCOPE(fn)` 的 `fn` 在**调用者**的执行上下文中同步求值;
//          只有 `SPAWN` 创建的任务体才是调度驱动的」
//
// ⇒ 判据:SCOPE 体能看到调用者的绑定(调用者上下文),并且 SCOPE 体的语句
// 在**任何子任务运行之前**执行(同步,不是被调度)。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h5_2_scope_body_is_evaluated_synchronously_in_the_caller_context() {
    let src = r#"
LET(x, 5);
SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() , PRINT("child"))));
  PRINT(x);
  PRINT("scope-body");
  0
));
PRINT("after-scope");
"#;
    let path = scratch_program("h5_2_scope_synchronous.wll", src);
    let out = run_wlwl(&path);
    // NEG-14
    let expected = "5\nscope-body\nchild\nafter-scope\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.5.2:SCOPE 体在调用者上下文里同步求值(看到 x,且先于子任务执行)",
    );
}

// ────────────────────────────────────────────────────────────────────────
// H.5.3 —— **SCOPE 退出**:作用域内的全部子任务达到终态之后,SCOPE 才返回。
//
// 规范依据(并发册):
//   H.5.3「前置  SCOPE 的 fn 已求值完毕」
//        「结论  作用域内的全部子任务达到终态之后,SCOPE 才返回(§17.1);
//               子任务的终值按 §17.1 AWAIT 交付」
//   §17.1「子任务寿命**不**超过词法 `SCOPE`;`SCOPE` 返回前**必须**等待其
//          子任务结束(结构化并发不变量)」
//
// ⇒ 判据:子任务的输出出现在 SCOPE 的返回值被使用**之前**。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h5_3_scope_returns_only_after_all_children_reach_a_terminal_state() {
    let src = r#"
LET(v, SCOPE(FUN(() ,
  LET(t, SPAWN(FUN(() , PRINT("child-done"); 7)));
  0
)));
PRINT(v);
"#;
    let path = scratch_program("h5_3_scope_waits_for_children.wll", src);
    let out = run_wlwl(&path);
    // NEG-15
    let expected = "child-done\n0\n";
    assert_stdout_eq(&out, expected, "H.5.3:SCOPE 在全部子任务达到终态之后才返回");
}

// ────────────────────────────────────────────────────────────────────────
// H.3.8 —— **调度器取任务**:从运行队列按**入队次序**取下一个可运行任务。
//
// 规范依据(并发册):
//   H.3.8「前置  Q 非空」
//        「结论  取 Q 的首条任务 τ;C' = C[ Q := Q \ τ ]」
//        「依据  §17.4 调度循环;§17.8 调度公平性」
//   §17.8「调度公平性:best-effort FIFO … 调度器从运行队列**按入队次序**取
//          下一个可运行任务」
//   §17.4「① 从运行队列按 FIFO 取出一个就绪任务(§17.8)」
//
// ⇒ 判据:先入队的任务先跑(入队次序 = a 然后 b)。取「队尾」的实现会把
// 两条输出反过来 ⇒ 这条断言能区分 FIFO 与 LIFO。
// ────────────────────────────────────────────────────────────────────────
#[test]
fn h3_8_scheduler_takes_the_head_of_the_run_queue_in_fifo_order() {
    let src = r#"
SCOPE(FUN(() ,
  LET(a, SPAWN(FUN(() , PRINT("a"))));
  LET(b, SPAWN(FUN(() , PRINT("b"))));
  AWAIT(a);
  AWAIT(b)
));
"#;
    let path = scratch_program("h3_8_fifo_take_head.wll", src);
    let out = run_wlwl(&path);
    // NEG-16
    let expected = "a\nb\n";
    assert_stdout_eq(
        &out,
        expected,
        "H.3.8:调度器按入队次序取运行队列首条任务(FIFO)",
    );
}

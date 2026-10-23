//! [v0.7 Phase D] Channel data structure for inter-task communication.
//!
//! See plan §5.3 (Channel semantics + close protocol) and §5.1.1
//! (state machine including `ReceivingOn`/`SendingOn` wait reasons).
//! See also `runtime::YieldReason::ReceivingOn` / `SendingOn` which
//! carry a `ChannelId` that addresses a slot in
//! `Scheduler::channels`.
pub use wlwl_value::{ChannelHandle, ChannelId};

use std::collections::VecDeque;

use crate::Value;

/// Identifies a channel slot within a single [`crate::runtime::Scheduler`].
///
/// Cheap to copy; not a handle (see [`ChannelHandle`] for what
/// `CHANNEL_NEW` returns to user code).
/// `CHANNEL_NEW(buf)`.
///
/// Detecting use-after-scope-exit: a channel slot is recycled when
/// Result of attempting a synchronous (non-suspending) channel op.
///
/// Distinct from `Yield` because TRY_* never yields — they only return
/// this enum and let the caller (a builtin) translate the outcome
/// into a user-facing value or error.
#[derive(Debug)]
pub enum TryResult<T> {
    /// Op completed synchronously with this value.
    Ok(T),
    /// Op could not complete without blocking. The caller must
    /// translate this to `Signal::Yield(ReceivingOn|SendingOn)`
    /// (or, for TRY_* builtins, surface a `Value::Boolean(false)`
    /// / `Value::Null`).
    WouldBlock,
    /// Channel is closed. For RECV, this carries the kind tag
    /// (`"ChannelClosed"`) per plan §5.3. For SEND on closed, the
    /// caller raises E0054 instead of returning this variant.
    Closed,
}

/// [X-01] 关闭时**被释放**的停泊等待方(并发册 §17.2.3 / 附录 H.3.7)。
///
/// ⚠ **两组都要给** —— §17.2.3:「关闭**必须**释放该通道上全部挂起的等待方」:
///
/// * 停泊的**接收方**被唤醒后,其 `RECV` 按「关闭且读空」完成,返回
///   `ERR(kind="ChannelClosed")`;
/// * 停泊的**发送方**被唤醒后,其 `SEND` 以 **`E0054`** 失败(发送到已关闭通道)。
///
/// ⇒ 唤醒次序**未定义**(§17.7)⇒ 两组各按自己的队列顺序交回去即可。
#[derive(Debug, Default)]
pub struct ClosedWaiters {
    /// 停泊的接收方(重跑后按「关闭且读空」完成)。
    pub receivers: Vec<crate::runtime::TaskId>,
    /// 停泊的发送方(重跑后其 `SEND` 以 `E0054` 失败)。
    pub senders: Vec<crate::runtime::TaskId>,
}

/// One channel slot.
///
/// Layout (plan §5.3):
/// - `buf`: a `VecDeque<Value>` with capacity `capacity`. `capacity
///   == 0` means synchronous (no buffering; SEND blocks until a
///   receiver picks up the value).
/// - `closed`: sticky close flag, set by `CHANNEL_CLOSE`. Once
///   `true`, SEND raises **E0054** (close + write) and RECV returns
///   `Value::Err` with `kind = "ChannelClosed"` payload.
/// - `sender_waiters` / `sender_waiter_values`: parallel arrays —
///   `sender_waiters[i]` is the i-th parked sender's task id;
///   `sender_waiter_values[i]` is the value it wanted to push.
///   For sync channels (cap=0), the value would have nowhere else to
///   live; for buffered channels (cap>0), the value would overflow
///   the buf so we stash it here. Both arrays push together and
///   pop together (FIFO).
/// - `receiver_waiters`: task ids parked because RECV found buf
///   empty (`ReceivingOn`). Filled by the runtime, not user code.
/// - `rendezvous`: `(sender, receiver)` task-id pairs that have already
///   completed a rendezvous on this channel, with how many times. Backs
///   the [v0.10.1 / R10-014] livelock guard — see
///   [`Channel::record_rendezvous`].
#[derive(Debug)]
pub struct Channel {
    pub id: ChannelId,
    pub generation: u64,
    pub capacity: usize,
    pub buf: VecDeque<Value>,
    pub closed: bool,
    pub sender_waiters: Vec<crate::runtime::TaskId>,
    pub sender_waiter_values: Vec<Value>,
    pub receiver_waiters: Vec<crate::runtime::TaskId>,
    pub rendezvous: Vec<(crate::runtime::TaskId, crate::runtime::TaskId, u32)>,
}

impl Channel {
    /// Construct a fresh channel with the given buffer capacity.
    /// The capacity is recorded verbatim; `0` means synchronous.
    pub fn new(id: ChannelId, generation: u64, capacity: usize) -> Self {
        Self {
            id,
            generation,
            capacity,
            buf: VecDeque::with_capacity(capacity),
            closed: false,
            sender_waiters: Vec::new(),
            sender_waiter_values: Vec::new(),
            receiver_waiters: Vec::new(),
            rendezvous: Vec::new(),
        }
    }

    /// The handle that user code receives from `CHANNEL_NEW`.
    pub fn handle(&self) -> ChannelHandle {
        ChannelHandle {
            id: self.id,
            generation: self.generation,
        }
    }

    /// `CHANNEL_LEN(ch)` — number of values currently buffered.
    /// After close, returns the residual buffer length (0 once
    /// receivers have drained it; the close flag itself is not
    /// surfaced here — `CHANNEL_CLOSE` is the only "is it closed"
    /// surface, see §5.3 note that NULL is NOT a close signal).
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Required by clippy's `len_without_is_empty` lint because we
    /// expose `len()`. The semantics differ slightly from a typical
    /// collection: a freshly-opened sync channel (buf=0) is "empty"
    /// before any value arrives AND has no capacity to fill, so this
    /// returns true any time the buffer holds zero items.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// `CHANNEL_CAP(ch)` — configured buffer capacity. `0` for sync.
    pub fn cap(&self) -> usize {
        self.capacity
    }

    /// True when there is room for a SEND without blocking.
    /// `capacity == 0` is always full (sync channel can't buffer).
    pub fn has_room(&self) -> bool {
        self.buf.len() < self.capacity
    }

    /// Synchronous SEND attempt. Used by `CHANNEL_TRY_SEND` and
    /// by the suspending `CHANNEL_SEND` before it falls through to
    /// the `WouldBlock` path (the latter routes via this helper to
    /// get a single `Ok/Closed/WouldBlock` answer and then decides
    /// whether to push, raise E0054, or yield).
    pub fn try_send(&mut self, v: Value) -> TryResult<()> {
        if self.closed {
            return TryResult::Closed;
        }
        if self.has_room() {
            self.buf.push_back(v);
            return TryResult::Ok(());
        }
        TryResult::WouldBlock
    }

    /// [v0.9 Step 4] Synchronous SEND with explicit pair logic.
    /// Returns Some(receiver_task_id) when pairing succeeds: a
    /// receiver was parked on a sync channel, the value has been
    /// pushed into the buf so the receiver's retry finds it, and
    /// the receiver task id is returned for the caller to wake.
    ///
    /// Sync-channel hand-off only — for buffered channels the
    /// natural `try_send` path is sufficient.
    pub fn try_pair_send(&mut self, value: Value) -> Option<crate::runtime::TaskId> {
        if self.closed {
            return None;
        }
        if self.capacity == 0 && self.has_receiver_waiter() {
            // Push the sender's value into the buf (which is empty
            // for sync channels) so the receiver's retry finds it.
            self.buf.push_back(value);
            return self.pop_receiver_waiter();
        }
        None
    }

    /// [v0.9 Step 4] Symmetric pair helper for the recv side.
    /// Returns Some((sender_task_id, sender_value)) when a sender is
    /// parked and its value is ready. Returns None when no sender is
    /// waiting (caller falls through to try_recv / WouldBlock).
    pub fn try_pair_recv(&mut self) -> Option<(crate::runtime::TaskId, Value)> {
        if self.closed {
            return None;
        }
        if !self.sender_waiters.is_empty() {
            return self.pop_sender_waiter_with_value();
        }
        None
    }

    /// Synchronous RECV attempt. Used by `CHANNEL_TRY_RECV` and
    /// by `CHANNEL_RECV`'s pre-yield probe.
    pub fn try_recv(&mut self) -> TryResult<Value> {
        if self.closed && self.buf.is_empty() {
            return TryResult::Closed;
        }
        match self.buf.pop_front() {
            Some(v) => TryResult::Ok(v),
            None => TryResult::WouldBlock,
        }
    }

    /// Build the structured `Value::Err` payload that RECV returns
    /// when the channel is closed and the buffer is drained. The
    /// shape is a Dict `{kind: "ChannelClosed", channel: <repr>}`
    /// — matching the spec note "NULL 不作为 close 信号;用户必须
    /// 用 ERR 检测".
    pub fn recv_closed_err(&self) -> Value {
        let repr = format!("<channel handle id={} gen={}>", self.id.0, self.generation);
        Value::Err(Box::new(Value::dict(vec![
            (
                Value::String("kind".to_string()),
                Value::String("ChannelClosed".to_string()),
            ),
            (Value::String("channel".to_string()), Value::String(repr)),
        ])))
    }

    /// Mark the channel closed. Idempotent. Returns the **parked waiters**
    /// that close releases, so the caller can re-enqueue them.
    ///
    /// ⚠ **[X-01] 两组都释放** —— 并发册 §17.2.3:「关闭**必须**释放该通道上
    /// 全部挂起的等待方」⇒ 停泊的**接收方**按「关闭且读空」完成(返回
    /// `ERR(kind="ChannelClosed")`);停泊的**发送方**重跑后其 `SEND` 以
    /// **`E0054`** 失败。
    ///
    /// ⇒ 此前**只**释放接收方 ⇒ 停泊的发送方永远不被唤醒 ⇒ 对它 `AWAIT` 得到
    /// `E0053`(无对端唤醒)⇒ **不**是规范要的 `E0054`(一致性册 §5.4 的 X-01)。
    ///
    /// ⚠ **`sender_waiter_values` 必须与 `sender_waiters` 一起换出** —— 两者是
    /// **平行数组**(一起 push、一起 pop)⇒ 只换出一个会让长度对不上 ⇒
    /// `pop_sender_waiter` 会取走**别人**的值 ⇒ 值错配。
    /// 停泊时存的那份值随发送方**重跑**作废(它会重新求值那个表达式)⇒ 直接丢弃。
    pub fn close(&mut self) -> ClosedWaiters {
        if self.closed {
            return ClosedWaiters::default();
        }
        self.closed = true;
        let mut receivers = Vec::new();
        std::mem::swap(&mut receivers, &mut self.receiver_waiters);
        let mut senders = Vec::new();
        std::mem::swap(&mut senders, &mut self.sender_waiters);
        let mut parked_values = Vec::new();
        std::mem::swap(&mut parked_values, &mut self.sender_waiter_values);
        drop(parked_values);
        ClosedWaiters { receivers, senders }
    }

    /// Wake one sender (FIFO) — returns its task id so the caller can
    /// re-enqueue. Called from `recv` after a value is dequeued (a
    /// sender was waiting because the buf was full; the freed slot
    /// lets it push and continue).
    pub fn pop_sender_waiter(&mut self) -> Option<crate::runtime::TaskId> {
        if self.sender_waiters.is_empty() {
            None
        } else {
            self.sender_waiter_values.remove(0);
            Some(self.sender_waiters.remove(0))
        }
    }

    /// [v0.9 Step 4] Wake one sender and return both its task id AND
    /// the value it parked with. The caller is responsible for either
    /// pushing the value into the buf (when a slot is now free) or
    /// handing it off to a paired receiver (sync channel path).
    pub fn pop_sender_waiter_with_value(&mut self) -> Option<(crate::runtime::TaskId, Value)> {
        if self.sender_waiters.is_empty() {
            None
        } else {
            let v = self.sender_waiter_values.remove(0);
            let tid = self.sender_waiters.remove(0);
            Some((tid, v))
        }
    }

    /// Wake one receiver (FIFO) — returns its task id so the caller
    /// can re-enqueue. Called from `send` after a value is enqueued
    /// (a receiver was waiting because the buf was empty; the new
    /// value lets it pop and continue).
    pub fn pop_receiver_waiter(&mut self) -> Option<crate::runtime::TaskId> {
        if self.receiver_waiters.is_empty() {
            None
        } else {
            Some(self.receiver_waiters.remove(0))
        }
    }

    /// Park the calling task as a sender waiter (the buf is full
    /// and SEND must yield). Caller is responsible for re-enqueuing
    /// the task on wake-up via `pop_sender_waiter`. The parked value
    /// is stored alongside the task id so a paired receiver can
    /// retrieve it on hand-off (sync channel path) or the woken
    /// sender can re-push into the buf after a slot frees.
    pub fn push_sender_waiter(&mut self, task: crate::runtime::TaskId, value: Value) {
        self.sender_waiters.push(task);
        self.sender_waiter_values.push(value);
    }

    /// Park the calling task as a receiver waiter (the buf is empty
    /// and RECV must yield).
    pub fn push_receiver_waiter(&mut self, task: crate::runtime::TaskId) {
        self.receiver_waiters.push(task);
    }

    /// True iff at least one receiver is parked waiting for a value.
    /// Used by SEND on a sync (buf=0) channel to decide whether to
    /// hand off directly or park the sender.
    pub fn has_receiver_waiter(&self) -> bool {
        !self.receiver_waiters.is_empty()
    }

    /// [v0.10.1 / R10-014] 记录一次 rendezvous,返回这是同一对任务的第几次。
    ///
    /// **为什么按「任务对」而不是按「通道」计数**:同一个通道上先后跑
    /// 两组互不相干的生产者/消费者(`p1`/`c1` 收工后再 `p2`/`c2`)是完全合法的,
    /// 按通道累计会把它误判。按任务对累计只抓「同一对任务在同一通道上
    /// rendezvous 了第二次」—— 而在当前的段重跑模型下,这**必然**是活锁。
    ///
    /// **为什么阈值是 2 而不是某个经验值**:通道操作挂起时**恢复点没存**
    /// ⇒ 被唤醒的任务从**段首**重跑(`Evaluator::run_task_segments` 取
    /// `segments[current_segment]` 再整段 `eval_block`)⇒ 段内副作用重复。
    /// ⚠⚠ **[2026-10-20 订正]此处原写「通道操作挂起时任务的 `running_env`
    /// 被丢弃……只有 `Yield(Explicit)` 才保存 env,其它 signal 一律丢弃」
    /// —— 与代码不符**:`run_task_segments` 的第 4 步是**无条件**把
    /// `env.scopes` 写回 `task.running_env`(`lib.rs` Step 4)⇒ **env 一直
    /// 在**;丢的是**恢复点**。于是:
    ///
    /// - 第一次 rendezvous:接收方停泊、发送方被唤醒后重跑,此时它看到有
    ///   停泊的接收者,`try_pair_send` 交接成功,SEND 正常结束 —— 能推进;
    /// - 第二次 rendezvous:发送方的循环变量已经回到初值,投出去的还是同一个
    ///   值,接收方重跑后又回到初值 —— 两边的状态与第一次**完全相同**,
    ///   永远不会推进。
    ///
    /// 所以「第二次」不是调出来的阈值,是当前架构的硬边界。真正修它需要
    /// 段内恢复(保留 env 并从挂起点之后继续),那是独立立项的事。
    pub fn record_rendezvous(
        &mut self,
        sender: crate::runtime::TaskId,
        receiver: crate::runtime::TaskId,
    ) -> u32 {
        if let Some(slot) = self
            .rendezvous
            .iter_mut()
            .find(|(s, r, _)| *s == sender && *r == receiver)
        {
            slot.2 += 1;
            slot.2
        } else {
            self.rendezvous.push((sender, receiver, 1));
            1
        }
    }

    /// 这一对任务之前是否已经在这条通道上 rendezvous 过。
    pub fn has_rendezvoused(
        &self,
        sender: crate::runtime::TaskId,
        receiver: crate::runtime::TaskId,
    ) -> bool {
        self.rendezvous
            .iter()
            .any(|(s, r, _)| *s == sender && *r == receiver)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;

    fn int(n: i64) -> Value {
        Value::Integer(n)
    }

    #[test]
    fn new_channel_is_open_with_zero_buf() {
        let c = Channel::new(ChannelId(0), 1, 0);
        assert_eq!(c.cap(), 0);
        assert_eq!(c.len(), 0);
        assert!(!c.closed);
        assert!(c.sender_waiters.is_empty());
        assert!(c.receiver_waiters.is_empty());
    }

    #[test]
    fn new_channel_records_capacity() {
        let c = Channel::new(ChannelId(0), 1, 8);
        assert_eq!(c.cap(), 8);
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn handle_round_trips() {
        let c = Channel::new(ChannelId(7), 3, 0);
        assert_eq!(
            c.handle(),
            ChannelHandle {
                id: ChannelId(7),
                generation: 3
            }
        );
    }

    #[test]
    fn two_handles_for_different_channels_differ() {
        let a = Channel::new(ChannelId(0), 1, 0).handle();
        let b = Channel::new(ChannelId(1), 1, 0).handle();
        assert_ne!(a, b);
    }

    #[test]
    fn same_id_different_generation_differ() {
        let a = Channel::new(ChannelId(0), 1, 0).handle();
        let b = Channel::new(ChannelId(0), 2, 0).handle();
        assert_ne!(a, b, "generation must differentiate handle reuse");
    }

    // ─── D-C op semantics ─────────────────────────────────────

    #[test]
    fn try_send_then_recv_round_trip() {
        let mut c = Channel::new(ChannelId(0), 1, 4);
        assert!(matches!(c.try_send(int(7)), TryResult::Ok(())));
        assert_eq!(c.len(), 1);
        match c.try_recv() {
            TryResult::Ok(v) => assert_eq!(v, int(7)),
            other => panic!("expected Ok, got {:?}", other),
        }
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn try_send_into_full_buf_yields_would_block() {
        let mut c = Channel::new(ChannelId(0), 1, 1);
        assert!(matches!(c.try_send(int(1)), TryResult::Ok(())));
        match c.try_send(int(2)) {
            TryResult::WouldBlock => {}
            other => panic!("expected WouldBlock, got {:?}", other),
        }
    }

    #[test]
    fn try_send_into_closed_yields_closed() {
        let mut c = Channel::new(ChannelId(0), 1, 4);
        c.close();
        match c.try_send(int(1)) {
            TryResult::Closed => {}
            other => panic!("expected Closed, got {:?}", other),
        }
    }

    #[test]
    fn try_recv_from_empty_yields_would_block() {
        let mut c = Channel::new(ChannelId(0), 1, 4);
        match c.try_recv() {
            TryResult::WouldBlock => {}
            other => panic!("expected WouldBlock, got {:?}", other),
        }
    }

    #[test]
    fn try_recv_from_closed_empty_returns_err_kind_channelclosed() {
        let c = Channel::new(ChannelId(0), 1, 4);
        let err = c.recv_closed_err();
        match err {
            Value::Err(payload) => match *payload {
                Value::Dict(entries) => {
                    let kind = entries.iter().find_map(|(k, v)| {
                        if matches!(k, Value::String(s) if s == "kind") {
                            Some(v.clone())
                        } else {
                            None
                        }
                    });
                    assert_eq!(kind, Some(Value::String("ChannelClosed".to_string())));
                }
                other => panic!("expected dict payload, got {:?}", other),
            },
            other => panic!("expected Err, got {:?}", other),
        }
    }

    #[test]
    fn close_returns_parked_receivers_for_wake() {
        let mut c = Channel::new(ChannelId(0), 1, 0);
        c.push_receiver_waiter(crate::runtime::TaskId(42));
        c.push_receiver_waiter(crate::runtime::TaskId(43));
        let closed = c.close();
        assert_eq!(
            closed.receivers,
            vec![crate::runtime::TaskId(42), crate::runtime::TaskId(43)]
        );
        assert!(closed.senders.is_empty());
        assert!(c.receiver_waiters.is_empty());
        assert!(c.closed);
    }

    /// [X-01] 关闭**必须**释放停泊的**发送方** ⇒ 它们的 `SEND` 重跑后以
    /// `E0054` 失败(并发册 §17.2.3 / 附录 H.3.7)。
    #[test]
    fn close_returns_parked_senders_for_wake() {
        let mut c = Channel::new(ChannelId(0), 1, 1);
        c.push_sender_waiter(crate::runtime::TaskId(7), int(1));
        c.push_sender_waiter(crate::runtime::TaskId(8), int(2));
        let closed = c.close();
        assert_eq!(
            closed.senders,
            vec![crate::runtime::TaskId(7), crate::runtime::TaskId(8)]
        );
        assert!(closed.receivers.is_empty());
        assert!(c.sender_waiters.is_empty());
    }

    /// ⚠ `sender_waiters` 与 `sender_waiter_values` 是**平行数组** ⇒ 关闭时
    /// 必须**一起**换出 ⇒ 否则长度对不上 ⇒ `pop_sender_waiter` 会取走**别人**的值。
    #[test]
    fn close_clears_the_parked_sender_values_in_lockstep() {
        let mut c = Channel::new(ChannelId(0), 1, 1);
        c.push_sender_waiter(crate::runtime::TaskId(7), int(1));
        c.push_sender_waiter(crate::runtime::TaskId(8), int(2));
        c.close();
        assert!(c.sender_waiters.is_empty());
        assert!(
            c.sender_waiter_values.is_empty(),
            "parked values must be cleared together with the waiters"
        );
        assert_eq!(c.sender_waiters.len(), c.sender_waiter_values.len());
    }

    #[test]
    fn close_idempotent_returns_empty_on_second_call() {
        let mut c = Channel::new(ChannelId(0), 1, 0);
        c.push_receiver_waiter(crate::runtime::TaskId(7));
        let first = c.close();
        assert_eq!(first.receivers, vec![crate::runtime::TaskId(7)]);
        let second = c.close();
        assert!(second.receivers.is_empty());
        assert!(second.senders.is_empty());
    }
}

//! [v0.7 Phase D] Channel data structure for inter-task communication.
//!
//! **Phase D-A: data shape only.** No behaviour lands here yet — the
//! [`Channel`] type, the wait queues, and the close protocol are
//! declared so reviewers can see the planned shape and so the rest
//! of the runtime (Phase D-B builtin wiring, Phase D-C scheduler
//! integration, Phase D-D leak detector) can build on top without
//! forward-declaration gymnastics. Behaviour arrives in subsequent
//! commits.
//!
//! See plan §5.3 (Channel semantics + close protocol) and §5.1.1
//! (state machine including `ReceivingOn`/`SendingOn` wait reasons).
//! See also `runtime::YieldReason::ReceivingOn` / `SendingOn` which
//! carry a `RcHandle` that addresses a slot in `Scheduler::channels`.

use std::collections::VecDeque;

use crate::Value;

/// Identifies a channel slot within a single [`crate::runtime::Scheduler`].
///
/// Cheap to copy; not a handle (see [`ChannelHandle`] for what
/// `CHANNEL_NEW` returns to user code).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelId(pub usize);

/// Generation-tracked channel handle returned to user code by
/// `CHANNEL_NEW(buf)`.
///
/// Detecting use-after-scope-exit: a channel slot is recycled when
/// the scope that owns it exits and the leak detector force-closes
/// the channel. The generation bumps on each recycle so a stale
/// handle fails E0053-style validation rather than operating on a
/// different channel than the one the user originally opened.
///
/// (At D-A this is a stub; the leak detector's bump_generation
/// wiring lands in D-D.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChannelHandle {
    pub id: ChannelId,
    pub generation: u64,
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
/// - `sender_waiters` / `receiver_waiters`: task ids parked because
///   SEND would overflow buf (`SendingOn`) or RECV found buf empty
///   (`ReceivingOn`). Both are filled by the runtime, not user code.
///   Phase D-A adds the type only — the queues stay empty until D-C
///   wires the scheduler.
#[derive(Debug)]
pub struct Channel {
    pub id: ChannelId,
    pub generation: u64,
    pub capacity: usize,
    pub buf: VecDeque<Value>,
    pub closed: bool,
    pub sender_waiters: Vec<crate::runtime::TaskId>,
    pub receiver_waiters: Vec<crate::runtime::TaskId>,
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
            receiver_waiters: Vec::new(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
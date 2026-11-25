use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

use super::ReplyError;

#[derive(Clone, Debug)]
pub(crate) struct Ticket(Arc<Admission>);

#[derive(Debug)]
struct Admission {
    state: AtomicU8,
    deadline: Instant,
}

#[repr(u8)]
enum State {
    Offered,
    Claimed,
    Expired,
}

impl Ticket {
    pub(crate) fn new(deadline: Instant) -> Self {
        Self(Arc::new(Admission {
            state: AtomicU8::new(State::Offered as u8),
            deadline,
        }))
    }

    pub(crate) fn claim(&self, at: Instant) -> bool {
        at < self.0.deadline && self.resolve(State::Claimed).is_ok()
    }

    pub(crate) fn timeout(&self) -> ReplyError {
        match self.resolve(State::Expired) {
            Err(state) if state == State::Claimed as u8 => ReplyError::OutcomeUnknown,
            _ => ReplyError::EngineBusy,
        }
    }

    fn resolve(&self, state: State) -> Result<u8, u8> {
        self.0.state.compare_exchange(
            State::Offered as u8,
            state as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
    }
}

impl PartialEq for Ticket {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Ticket {}

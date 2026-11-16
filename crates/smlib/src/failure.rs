//! The code of the last kernel call that failed on this thread (RFC 0016): past this crate the
//! error is only text, so the code is read here, beside it.

use std::cell::Cell;

use solidcore::SolidError;
pub use solidcore_ops::FailureCode;

thread_local! {
    static CALL: Cell<Option<&'static str>> = const { Cell::new(None) };
    static LAST: Cell<Option<FailureCode>> = const { Cell::new(None) };
}

/// The code of the last kernel call that failed on this thread since [`clear`].
pub fn last() -> Option<FailureCode> {
    LAST.with(Cell::get)
}

pub fn clear() {
    LAST.with(|l| l.set(None));
}

pub(crate) fn record(e: &SolidError) {
    if let Some(call) = CALL.with(Cell::get) {
        LAST.with(|l| l.set(Some(FailureCode { call, kind: e.kind().code(), operation: e.operation() })));
    }
}

/// The kernel call running on this thread, while this lives.
pub(crate) struct InCall(Option<&'static str>);

impl InCall {
    pub(crate) fn enter(call: &'static str) -> Self {
        Self(CALL.with(|c| c.replace(Some(call))))
    }
}

impl Drop for InCall {
    fn drop(&mut self) {
        CALL.with(|c| c.set(self.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_recorded_under_its_call_and_cleared() {
        clear();
        let e = SolidError::InvalidInput { operation: "test::op", reason: "no" };
        record(&e);
        assert_eq!(last(), None, "outside a call nothing is recorded");
        {
            let _in = InCall::enter("outer");
            {
                let _inner = InCall::enter("inner");
                record(&e);
            }
            assert_eq!(last().map(|c| c.call), Some("inner"));
            record(&e);
            assert_eq!(last(), Some(FailureCode { call: "outer", kind: e.kind().code(), operation: e.operation() }));
        }
        clear();
        assert_eq!(last(), None);
    }
}

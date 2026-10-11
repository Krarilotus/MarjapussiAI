//! Opt-in, thread-local elapsed-work timing. Disabled builds have zero-sized scopes.
//! Inclusive time includes nested scopes; exclusive time subtracts their time.
//! Profiling adds overhead and must not be used as the final speed measurement.

#[cfg(feature = "profile")]
mod enabled {
    use std::{cell::RefCell, collections::BTreeMap, time::Instant};

    #[derive(Clone, Default, Debug)]
    pub struct Timing {
        pub calls: u64,
        pub inclusive_ns: u128,
        pub exclusive_ns: u128,
    }

    #[derive(Default)]
    struct State {
        children: Vec<u128>,
        timings: BTreeMap<&'static str, Timing>,
    }

    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    #[must_use]
    pub struct Scope {
        name: &'static str,
        start: Instant,
    }

    pub fn scope(name: &'static str) -> Scope {
        STATE.with(|s| s.borrow_mut().children.push(0));
        Scope {
            name,
            start: Instant::now(),
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            let elapsed = self.start.elapsed().as_nanos();
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                let child = s.children.pop().expect("balanced timing scopes");
                if let Some(parent) = s.children.last_mut() {
                    *parent += elapsed;
                }
                let t = s.timings.entry(self.name).or_default();
                t.calls += 1;
                t.inclusive_ns += elapsed;
                t.exclusive_ns += elapsed.saturating_sub(child);
            });
        }
    }

    /// Read completed scopes on this thread. Active scopes are not included.
    pub fn snapshot() -> BTreeMap<&'static str, Timing> {
        STATE.with(|s| s.borrow().timings.clone())
    }

    /// Reset between measurements, outside all timing scopes.
    pub fn reset() {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert!(s.children.is_empty(), "cannot reset active timing scopes");
            s.timings.clear();
        });
    }
}

#[cfg(feature = "profile")]
pub use enabled::*;

#[cfg(not(feature = "profile"))]
#[must_use]
pub struct Scope;

#[cfg(not(feature = "profile"))]
#[inline(always)]
pub fn scope(_: &'static str) -> Scope {
    Scope
}

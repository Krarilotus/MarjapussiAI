//! Wall-clock time for events and meta info.

use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::offset::Local;
use chrono::DateTime;

/// Source of `GameEvent::time`. Reading the wall clock costs about 30 ns per action
/// (docs/design/performance.md); simulations switch it off.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Clock {
    #[default]
    Wall,
    Off,
}

/// Milliseconds since the Unix epoch (0 if the clock is before it).
pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A `GameEvent::time` as local time, `YYYY-MM-DD HH:MM:SS`.
pub fn format_time(millis: u64) -> String {
    let time = UNIX_EPOCH + std::time::Duration::from_millis(millis);
    let datetime: DateTime<Local> = time.into();
    format!("{}", datetime.format("%Y-%m-%d %T"))
}

/// Local wall-clock time as `YYYY-MM-DD HH:MM:SS`. The text is formatted once per second and
/// per thread and then reused: formatting local time costs more than a whole card play.
pub fn current_time_string() -> String {
    thread_local! {
        static CACHE: RefCell<(u64, String)> = const { RefCell::new((u64::MAX, String::new())) };
    }
    let system_time = SystemTime::now();
    let second = system_time
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX - 1);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.0 != second {
            let datetime: DateTime<Local> = system_time.into();
            *cache = (second, format!("{}", datetime.format("%Y-%m-%d %T")));
        }
        cache.1.clone()
    })
}

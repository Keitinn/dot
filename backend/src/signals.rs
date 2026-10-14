//! Signals read from the OS. Kept apart from the pet logic so tests never touch the OS.

use std::sync::Once;

/// Seconds since the last keyboard, mouse or trackpad input in this login session (design F5).
/// Needs no permission on macOS. Returns 0 (present) where it cannot be read, so a failure
/// never puts dot into deep sleep; the failure is logged once instead of passing silently.
pub fn idle_seconds() -> u64 {
    match platform::idle_seconds().and_then(whole_seconds) {
        Some(secs) => secs,
        None => {
            static WARN: Once = Once::new();
            WARN.call_once(|| eprintln!("Input idle time is unavailable; treating the user as present"));
            0
        }
    }
}

/// NaN, negative or infinite readings are unusable.
fn whole_seconds(secs: f64) -> Option<u64> {
    (secs.is_finite() && secs >= 0.0).then_some(secs as u64)
}

#[cfg(target_os = "macos")]
mod platform {
    // CoreGraphics/CGEventSource.h and CGEventTypes.h
    type CGEventSourceStateID = i32;
    type CGEventType = u32;
    const COMBINED_SESSION_STATE: CGEventSourceStateID = 0; // kCGEventSourceStateCombinedSessionState
    const ANY_INPUT_EVENT_TYPE: CGEventType = !0; // kCGAnyInputEventType

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: CGEventSourceStateID, event_type: CGEventType) -> f64;
    }

    pub fn idle_seconds() -> Option<f64> {
        // SAFETY: plain C function taking two integers and returning a double; no pointers involved.
        Some(unsafe { CGEventSourceSecondsSinceLastEventType(COMBINED_SESSION_STATE, ANY_INPUT_EVENT_TYPE) })
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn idle_seconds() -> Option<f64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whole_seconds_truncates() {
        assert_eq!(whole_seconds(12.9), Some(12));
        assert_eq!(whole_seconds(0.0), Some(0));
    }

    #[test]
    fn test_unusable_readings_are_rejected() {
        assert_eq!(whole_seconds(f64::NAN), None);
        assert_eq!(whole_seconds(-3.0), None);
        assert_eq!(whole_seconds(f64::INFINITY), None);
    }
}

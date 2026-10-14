//! Companion settings and the onboarding answer (design/gdd/companion.md R1).

use chrono::{NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use super::consts::*;
use super::state::{Onboarding, Settings};

/// QUIET 1H / QUIET TODAY, or back to normal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum QuietAction {
    #[serde(rename = "quiet_1h")]
    Quiet1h,
    #[serde(rename = "quiet_today")]
    QuietToday,
    #[serde(rename = "off")]
    Off,
}

/// A change from the settings menu. Fields left out stay as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SettingsUpdate {
    pub master_on: Option<bool>,
    /// Local hour, 0–23.
    pub quiet_start: Option<u8>,
    /// Local hour, 0–23.
    pub quiet_end: Option<u8>,
    pub quiet: Option<QuietAction>,
}

/// What the settings menu shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SettingsView {
    pub master_on: bool,
    pub quiet_start: u8,
    pub quiet_end: u8,
    /// Unix seconds QUIET 1H / QUIET TODAY lasts until, while it is on.
    pub quiet_until: Option<i64>,
}

impl Settings {
    /// Master OFF, or QUIET 1H / QUIET TODAY still running: no lines.
    pub fn is_muted(&self, now: i64) -> bool {
        let _ = now;
        todo!()
    }

    pub fn view(&self, now: i64) -> SettingsView {
        let _ = now;
        todo!()
    }
}

/// Applies `update`, or nothing at all when any part of it is invalid.
/// `local` is the local time matching `now`, for QUIET TODAY.
pub fn apply(settings: &mut Settings, update: &SettingsUpdate, now: i64, local: NaiveDateTime) -> Result<(), String> {
    let _ = (settings, update, now, local);
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingAnswer {
    Yes,
    Later,
}

impl OnboardingAnswer {
    pub fn parse(answer: &str) -> Result<Self, String> {
        let _ = answer;
        todo!()
    }
}

/// YES or LATER on `STAY WITH YOU?`. LATER after the first prompt keeps the
/// one re-prompt LATER_REPROMPT_D days on; LATER after that one ends it.
pub fn answer_onboarding(onboarding: &mut Onboarding, answer: OnboardingAnswer, now: i64) {
    let _ = (onboarding, answer, now);
    todo!()
}

/// Called on each open: whether to show `STAY WITH YOU?` now. Showing it
/// counts as LATER until answered, so ignoring it is the same as LATER.
pub fn onboarding_at_open(onboarding: &mut Onboarding, now: i64) -> bool {
    let _ = (onboarding, now);
    todo!()
}

/// Seconds the reprompt waits.
const LATER_SECS: i64 = LATER_REPROMPT_D * 24 * 3600;

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    const T0: i64 = 1_800_000_000;
    const DAY: i64 = 24 * 3600;

    fn local(hour: u32, min: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 14).unwrap().and_hms_opt(hour, min, 0).unwrap()
    }

    fn update() -> SettingsUpdate {
        SettingsUpdate::default()
    }

    // Settings

    #[test]
    fn hours_and_master_switch_are_applied() {
        let mut s = Settings::default();
        let u = SettingsUpdate { master_on: Some(false), quiet_start: Some(0), quiet_end: Some(23), ..update() };
        apply(&mut s, &u, T0, local(12, 0)).unwrap();
        assert!(!s.master_on);
        assert_eq!((s.quiet_start, s.quiet_end), (0, 23));
    }

    #[test]
    fn hours_outside_0_to_23_are_refused_and_nothing_changes() {
        let mut s = Settings::default();
        let u = SettingsUpdate { master_on: Some(false), quiet_start: Some(5), quiet_end: Some(24), ..update() };
        assert!(apply(&mut s, &u, T0, local(12, 0)).is_err());
        assert_eq!(s, Settings::default());
        let u = SettingsUpdate { quiet_start: Some(200), ..update() };
        assert!(apply(&mut s, &u, T0, local(12, 0)).is_err());
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn an_empty_update_changes_nothing() {
        let mut s = Settings::default();
        apply(&mut s, &update(), T0, local(12, 0)).unwrap();
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn quiet_1h_mutes_for_an_hour() {
        let mut s = Settings::default();
        let u = SettingsUpdate { quiet: Some(QuietAction::Quiet1h), ..update() };
        apply(&mut s, &u, T0, local(12, 0)).unwrap();
        assert_eq!(s.quiet_until, Some(T0 + 3600));
        assert!(s.is_muted(T0 + 3599));
        assert!(!s.is_muted(T0 + 3600));
    }

    #[test]
    fn quiet_today_mutes_until_local_midnight() {
        let mut s = Settings::default();
        let u = SettingsUpdate { quiet: Some(QuietAction::QuietToday), ..update() };
        apply(&mut s, &u, T0, local(21, 30)).unwrap();
        assert_eq!(s.quiet_until, Some(T0 + 2 * 3600 + 30 * 60));
    }

    #[test]
    fn quiet_off_ends_the_mute() {
        let mut s = Settings { quiet_until: Some(T0 + 100), ..Settings::default() };
        let u = SettingsUpdate { quiet: Some(QuietAction::Off), ..update() };
        apply(&mut s, &u, T0, local(12, 0)).unwrap();
        assert_eq!(s.quiet_until, None);
        assert!(!s.is_muted(T0));
    }

    #[test]
    fn master_off_mutes() {
        let s = Settings { master_on: false, ..Settings::default() };
        assert!(s.is_muted(T0));
        assert!(!Settings::default().is_muted(T0));
    }

    #[test]
    fn the_view_hides_an_expired_quiet() {
        let s = Settings { quiet_until: Some(T0), ..Settings::default() };
        assert_eq!(s.view(T0 - 1).quiet_until, Some(T0));
        assert_eq!(s.view(T0).quiet_until, None);
        assert_eq!((s.view(T0).quiet_start, s.view(T0).quiet_end), (22, 8));
    }

    #[test]
    fn updates_parse_from_the_ui_and_refuse_unknown_values() {
        let u: SettingsUpdate = serde_json::from_str(r#"{"quiet":"quiet_1h","quiet_start":21}"#).unwrap();
        assert_eq!(u, SettingsUpdate { quiet: Some(QuietAction::Quiet1h), quiet_start: Some(21), ..update() });
        assert!(serde_json::from_str::<SettingsUpdate>(r#"{"quiet":"forever"}"#).is_err());
        assert!(serde_json::from_str::<SettingsUpdate>(r#"{"quiet_start":-1}"#).is_err());
        assert!(serde_json::from_str::<SettingsUpdate>(r#"{"volume":3}"#).is_err());
    }

    // Onboarding

    #[test]
    fn the_first_open_as_an_adult_prompts() {
        let mut o = Onboarding::Unseen;
        assert!(onboarding_at_open(&mut o, T0));
        assert_eq!(o, Onboarding::Later { until: T0 + 7 * DAY });
    }

    #[test]
    fn later_waits_7_days_then_prompts_once() {
        let mut o = Onboarding::Unseen;
        onboarding_at_open(&mut o, T0);
        answer_onboarding(&mut o, OnboardingAnswer::Later, T0 + 10);
        assert_eq!(o, Onboarding::Later { until: T0 + 7 * DAY });

        assert!(!onboarding_at_open(&mut o, T0 + DAY));
        assert!(!onboarding_at_open(&mut o, T0 + 7 * DAY - 1));
        assert!(onboarding_at_open(&mut o, T0 + 7 * DAY));
        assert!(!onboarding_at_open(&mut o, T0 + 7 * DAY + 1));
        assert!(!onboarding_at_open(&mut o, T0 + 30 * DAY));
    }

    #[test]
    fn a_second_later_ends_the_prompts() {
        let mut o = Onboarding::Unseen;
        onboarding_at_open(&mut o, T0);
        answer_onboarding(&mut o, OnboardingAnswer::Later, T0);
        assert!(onboarding_at_open(&mut o, T0 + 8 * DAY));
        answer_onboarding(&mut o, OnboardingAnswer::Later, T0 + 8 * DAY);
        assert_eq!(o, Onboarding::Done);
        assert!(!onboarding_at_open(&mut o, T0 + 20 * DAY));
    }

    #[test]
    fn yes_ends_the_prompts() {
        let mut o = Onboarding::Unseen;
        onboarding_at_open(&mut o, T0);
        answer_onboarding(&mut o, OnboardingAnswer::Yes, T0);
        assert_eq!(o, Onboarding::Yes);
        assert!(!onboarding_at_open(&mut o, T0 + 8 * DAY));
    }

    #[test]
    fn yes_on_the_reprompt_is_kept() {
        let mut o = Onboarding::Later { until: T0 };
        assert!(onboarding_at_open(&mut o, T0));
        answer_onboarding(&mut o, OnboardingAnswer::Yes, T0);
        assert_eq!(o, Onboarding::Yes);
    }

    #[test]
    fn later_before_any_prompt_waits_7_days() {
        let mut o = Onboarding::Unseen;
        answer_onboarding(&mut o, OnboardingAnswer::Later, T0);
        assert_eq!(o, Onboarding::Later { until: T0 + 7 * DAY });
    }

    #[test]
    fn answers_parse() {
        assert_eq!(OnboardingAnswer::parse("yes"), Ok(OnboardingAnswer::Yes));
        assert_eq!(OnboardingAnswer::parse("later"), Ok(OnboardingAnswer::Later));
        assert!(OnboardingAnswer::parse("YES").is_err());
        assert!(OnboardingAnswer::parse("maybe").is_err());
    }
}

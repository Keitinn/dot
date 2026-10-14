//! What the companion remembers between launches (`companion.json`).

use std::collections::BTreeMap;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use crate::pet::{EvolutionStage, PetState};
use super::bond::{self, DayCounters};
use super::consts::*;

/// Every field falls back to its default, so saves from older or newer
/// versions with missing fields still load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CompanionState {
    pub schema_version: u32,
    /// The pet this Bond belongs to. A different id (the pet save was reset)
    /// starts the companion over, like a revive.
    pub pet_id: Option<String>,
    /// 0–1000. Never shown in the UI.
    pub bond: u16,
    /// Counters since the last settlement. Kept as one value so a future
    /// sync can record them per device and per date.
    pub today: DayCounters,
    /// Local date of the last settlement. `None` until the pet is an adult.
    pub last_settle_date: Option<NaiveDate>,
    /// Unix seconds of the last manual open.
    pub last_open_at: Option<i64>,
    /// Local date of the last manual open; `m` in F1 counts days from it.
    pub last_open_date: Option<NaiveDate>,
    /// Overdue days already taken off Bond.
    pub overdue_applied: u32,
    pub settings: Settings,
    pub lines: LineHistory,
    pub onboarding: Onboarding,
}

/// Per-device settings. Survive a revive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub master_on: bool,
    /// Quiet hours, local hour. Start inclusive, end exclusive; equal means none.
    pub quiet_start: u8,
    pub quiet_end: u8,
    /// Unix seconds until which QUIET 1H / QUIET TODAY mutes the companion.
    pub quiet_until: Option<i64>,
}

/// Lines already said, so they are not repeated.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineHistory {
    /// Newest last, at most LINE_HISTORY.
    pub recent_ids: Vec<String>,
    /// Unix seconds each category last spoke.
    pub category_last_at: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Onboarding {
    #[default]
    Unseen,
    /// Asked again once after `until` (unix seconds).
    Later { until: i64 },
    Yes,
    /// Also what an unknown value from a newer version reads as, so it never asks again.
    #[serde(other)]
    Done,
}

/// The care actions that count towards Bond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CareKind {
    Feed,
    Play,
    Sleep,
}

impl CareKind {
    /// The need this action fills.
    pub fn need(self, pet: &PetState) -> u8 {
        let _ = pet;
        unimplemented!()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            master_on: true,
            quiet_start: QUIET_START_DEFAULT,
            quiet_end: QUIET_END_DEFAULT,
            quiet_until: None,
        }
    }
}

impl Default for CompanionState {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            pet_id: None,
            bond: 0,
            today: DayCounters::default(),
            last_settle_date: None,
            last_open_at: None,
            last_open_date: None,
            overdue_applied: 0,
            settings: Settings::default(),
            lines: LineHistory::default(),
            onboarding: Onboarding::Unseen,
        }
    }
}

impl CompanionState {
    /// Brings the companion up to `today`: starts it the first time the pet
    /// is seen as an adult, then settles Bond once per new day. Does nothing
    /// unless the pet is a living adult.
    pub fn advance_day(&mut self, pet: &PetState, today: NaiveDate) {
        let _ = (pet, today);
        unimplemented!()
    }

    /// Settles the counters into Bond if `today` is after the last settlement.
    /// Returns whether it did. A date at or before the last one does nothing.
    pub fn settle(&mut self, today: NaiveDate) -> bool {
        let _ = today;
        unimplemented!()
    }

    /// A manual open of the popover (tray click).
    pub fn record_open(&mut self, pet: &PetState, now: i64, today: NaiveDate) {
        let _ = (pet, now, today);
        unimplemented!()
    }

    /// A successful FEED / PLAY / SLEEP. `need_before` is the need the action
    /// fills, read just before it; only low needs count, so spamming a full
    /// pet does not grow Bond.
    pub fn record_care(&mut self, pet: &PetState, need_before: u8, today: NaiveDate) {
        let _ = (pet, need_before, today);
        unimplemented!()
    }

    /// The pet was revived: the new pet starts with no Bond, no counters and
    /// no line history, and is asked again when it grows up. Settings stay.
    pub fn reset_for_new_pet(&mut self) {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn adult() -> PetState {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Adult;
        pet
    }

    /// Settled through d0 (Oct 1), last opened on d0, with `bond`.
    fn companion_for(pet: &PetState, bond: u16) -> CompanionState {
        let mut companion = CompanionState::default();
        companion.advance_day(pet, date(1));
        companion.bond = bond;
        companion
    }

    #[test]
    fn existing_adult_without_a_save_starts_at_zero_today() {
        let pet = adult();
        let mut companion = CompanionState::default();

        companion.advance_day(&pet, date(14));

        assert_eq!(companion.bond, 0);
        assert_eq!(companion.last_settle_date, Some(date(14)));
        assert_eq!(companion.last_open_date, Some(date(14)));
        assert_eq!(companion.overdue_applied, 0);
        assert_eq!(companion.onboarding, Onboarding::Unseen);
        assert_eq!(companion.today, DayCounters::default());
        assert_eq!(companion.pet_id.as_deref(), Some(pet.id.as_str()));
    }

    #[test]
    fn nothing_starts_before_adult() {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Child;
        let mut companion = CompanionState::default();

        companion.advance_day(&pet, date(14));

        assert_eq!(companion, CompanionState::default());
    }

    #[test]
    fn a_dead_adult_does_not_settle() {
        let mut pet = adult();
        let mut companion = companion_for(&pet, 280);
        pet.is_alive = false;

        companion.advance_day(&pet, date(9));

        assert_eq!(companion.bond, 280);
        assert_eq!(companion.last_settle_date, Some(date(1)));
    }

    #[test]
    fn f1_example_running_every_day() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);

        // d0 = Oct 1, so dN = Oct N+1
        for (d, expected) in [(2, 280), (3, 280), (4, 280), (5, 272), (6, 264)] {
            companion.advance_day(&pet, date(d));
            assert_eq!(companion.bond, expected, "after Oct {}", d);
        }
    }

    #[test]
    fn f1_example_closed_until_d5() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);

        companion.advance_day(&pet, date(6));

        assert_eq!(companion.bond, 264);
        assert_eq!(companion.overdue_applied, 2);
    }

    #[test]
    fn greeting_feed_and_focus_on_d5_settle_to_292_on_d6() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);
        companion.advance_day(&pet, date(6));
        assert_eq!(companion.bond, 264);

        companion.record_open(&pet, 1_000, date(6));
        companion.record_care(&pet, 50, date(6));
        companion.today.n_focus = 1;
        companion.advance_day(&pet, date(7));

        assert_eq!(companion.bond, 292);
        assert_eq!(companion.today, DayCounters::default());
    }

    #[test]
    fn opening_clears_overdue_applied_and_moves_m() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);
        companion.advance_day(&pet, date(6));
        assert_eq!(companion.overdue_applied, 2);

        companion.record_open(&pet, 1_234, date(6));

        assert_eq!(companion.overdue_applied, 0);
        assert_eq!(companion.last_open_date, Some(date(6)));
        assert_eq!(companion.last_open_at, Some(1_234));
        assert!(companion.today.g);
    }

    #[test]
    fn opening_on_a_new_day_settles_the_previous_counters_first() {
        let pet = adult();
        let mut companion = companion_for(&pet, 100);
        companion.record_care(&pet, 10, date(1));

        companion.record_open(&pet, 0, date(2));

        assert_eq!(companion.bond, 103);
        assert_eq!(companion.today, DayCounters { g: true, n_care: 0, n_focus: 0 });
    }

    #[test]
    fn a_day_gains_at_most_50() {
        let pet = adult();
        let mut companion = companion_for(&pet, 0);
        companion.record_open(&pet, 0, date(1));
        for _ in 0..10 {
            companion.record_care(&pet, 10, date(1));
        }
        companion.today.n_focus = 3;

        companion.advance_day(&pet, date(2));

        assert_eq!(companion.bond, 50);
    }

    #[test]
    fn care_counts_only_five_times_a_day() {
        let pet = adult();
        let mut companion = companion_for(&pet, 0);
        for _ in 0..8 {
            companion.record_care(&pet, 10, date(1));
        }

        companion.advance_day(&pet, date(2));

        assert_eq!(companion.bond, 15);
    }

    #[test]
    fn same_day_settles_only_once() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);
        companion.record_care(&pet, 10, date(1));

        assert!(companion.settle(date(2)));
        assert_eq!(companion.bond, 283);
        companion.record_care(&pet, 10, date(2));
        assert!(!companion.settle(date(2)));

        assert_eq!(companion.bond, 283);
        assert_eq!(companion.today.n_care, 1);
    }

    #[test]
    fn a_date_before_the_last_settlement_does_not_settle() {
        let pet = adult();
        let mut companion = companion_for(&pet, 280);
        companion.advance_day(&pet, date(10));
        let before = companion.clone();
        companion.record_care(&pet, 10, date(10));

        assert!(!companion.settle(date(8)));
        companion.advance_day(&pet, date(8));

        assert_eq!(companion.bond, before.bond);
        assert_eq!(companion.last_settle_date, Some(date(10)));
        assert_eq!(companion.today.n_care, 1);
    }

    #[test]
    fn care_with_a_low_need_counts() {
        let pet = adult();
        let mut companion = companion_for(&pet, 0);

        companion.record_care(&pet, 79, date(1));

        assert_eq!(companion.today.n_care, 1);
    }

    #[test]
    fn care_with_a_need_of_80_or_more_does_not_count() {
        let pet = adult();
        let mut companion = companion_for(&pet, 0);

        companion.record_care(&pet, 80, date(1));
        companion.record_care(&pet, 100, date(1));

        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn care_before_adult_does_not_count() {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Child;
        let mut companion = CompanionState::default();

        companion.record_care(&pet, 10, date(1));
        companion.record_open(&pet, 0, date(1));

        assert_eq!(companion.today, DayCounters::default());
    }

    #[test]
    fn care_for_a_dead_pet_does_not_count() {
        let mut pet = adult();
        let mut companion = companion_for(&pet, 0);
        pet.is_alive = false;

        companion.record_care(&pet, 10, date(1));

        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn care_kind_picks_the_need_it_fills() {
        let mut pet = adult();
        pet.hunger = 11;
        pet.happiness = 22;
        pet.energy = 33;

        assert_eq!(CareKind::Feed.need(&pet), 11);
        assert_eq!(CareKind::Play.need(&pet), 22);
        assert_eq!(CareKind::Sleep.need(&pet), 33);
    }

    #[test]
    fn revive_resets_bond_counters_and_history_but_keeps_settings() {
        let pet = adult();
        let mut companion = companion_for(&pet, 420);
        companion.record_open(&pet, 5, date(1));
        companion.record_care(&pet, 10, date(1));
        companion.lines.recent_ids.push("greet_1".to_string());
        companion.lines.category_last_at.insert("greet_first_today".to_string(), 5);
        companion.onboarding = Onboarding::Yes;
        companion.settings.quiet_start = 21;
        companion.settings.master_on = false;

        companion.reset_for_new_pet();

        assert_eq!(companion.bond, 0);
        assert_eq!(companion.today, DayCounters::default());
        assert_eq!(companion.lines, LineHistory::default());
        assert_eq!(companion.onboarding, Onboarding::Unseen);
        assert_eq!(companion.last_settle_date, None);
        assert_eq!(companion.last_open_date, None);
        assert_eq!(companion.last_open_at, None);
        assert_eq!(companion.overdue_applied, 0);
        assert_eq!(companion.pet_id, None);
        assert_eq!(companion.settings.quiet_start, 21);
        assert!(!companion.settings.master_on);
    }

    #[test]
    fn revived_pet_starts_over_when_it_grows_up() {
        let mut pet = adult();
        let mut companion = companion_for(&pet, 420);
        companion.reset_for_new_pet();
        pet.stage = EvolutionStage::Egg;
        companion.advance_day(&pet, date(3));
        assert_eq!(companion.last_settle_date, None);

        pet.stage = EvolutionStage::Adult;
        companion.advance_day(&pet, date(20));

        assert_eq!(companion.bond, 0);
        assert_eq!(companion.last_settle_date, Some(date(20)));
        assert_eq!(companion.last_open_date, Some(date(20)));
    }

    #[test]
    fn a_different_pet_starts_over() {
        let pet = adult();
        let mut companion = companion_for(&pet, 420);
        let other = adult();

        companion.advance_day(&other, date(2));

        assert_eq!(companion.bond, 0);
        assert_eq!(companion.pet_id.as_deref(), Some(other.id.as_str()));
        assert_eq!(companion.last_settle_date, Some(date(2)));
    }

    #[test]
    fn defaults_match_the_design() {
        let companion = CompanionState::default();
        assert_eq!(companion.schema_version, 1);
        assert!(companion.settings.master_on);
        assert_eq!((companion.settings.quiet_start, companion.settings.quiet_end), (22, 8));
        assert_eq!(companion.settings.quiet_until, None);
        assert_eq!(companion.onboarding, Onboarding::Unseen);
    }

    #[test]
    fn round_trips_through_json() {
        let pet = adult();
        let mut companion = companion_for(&pet, 321);
        companion.record_open(&pet, 99, date(1));
        companion.onboarding = Onboarding::Later { until: 1_800_000_000 };
        companion.settings.quiet_until = Some(42);
        companion.lines.recent_ids.push("x".to_string());

        let json = serde_json::to_string(&companion).unwrap();
        let back: CompanionState = serde_json::from_str(&json).unwrap();

        assert_eq!(back, companion);
        assert!(json.contains("\"last_settle_date\":\"2026-10-01\""), "{}", json);
    }

    #[test]
    fn an_older_save_with_missing_fields_loads() {
        let back: CompanionState = serde_json::from_str(
            r#"{"bond":120,"today":{"g":true},"last_settle_date":"2026-10-05","settings":{"quiet_start":23}}"#,
        )
        .unwrap();

        assert_eq!(back.bond, 120);
        assert!(back.today.g);
        assert_eq!(back.today.n_care, 0);
        assert_eq!(back.last_settle_date, Some(date(5)));
        assert_eq!(back.settings.quiet_start, 23);
        assert_eq!(back.settings.quiet_end, 8);
        assert!(back.settings.master_on);
        assert_eq!(back.onboarding, Onboarding::Unseen);
    }

    #[test]
    fn an_empty_object_loads_as_defaults() {
        let back: CompanionState = serde_json::from_str("{}").unwrap();
        assert_eq!(back, CompanionState::default());
    }

    #[test]
    fn an_unknown_onboarding_state_reads_as_done() {
        let back: CompanionState =
            serde_json::from_str(r#"{"onboarding":{"state":"asked_twice"}}"#).unwrap();
        assert_eq!(back.onboarding, Onboarding::Done);
    }
}

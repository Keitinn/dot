//! Mood (design/gdd/companion.md R2 / F2): the first row of the R2 table that
//! matches. Pure: the clock, the hour and the time since the last open come in
//! as arguments.

use serde::{Deserialize, Serialize};
use crate::pet::{state::Activity, PetState};
use super::consts::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood {
    Concerned,
    Sleepy,
    MissedYou,
    Cheerful,
    /// Also what an unknown mood from a newer version reads as.
    #[serde(other)]
    Calm,
}

impl Mood {
    pub fn as_str(self) -> &'static str {
        match self {
            Mood::Concerned => "concerned",
            Mood::Sleepy => "sleepy",
            Mood::MissedYou => "missed_you",
            Mood::Calm => "calm",
            Mood::Cheerful => "cheerful",
        }
    }
}

/// What `concerned` is about. Lines about the same need are spaced out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Need {
    Hunger,
    Happiness,
    Energy,
    Health,
}

impl Need {
    pub fn as_str(self) -> &'static str {
        match self {
            Need::Hunger => "hunger",
            Need::Happiness => "happiness",
            Need::Energy => "energy",
            Need::Health => "health",
        }
    }
}

/// Everything the mood depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoodInput {
    pub hunger: u8,
    pub happiness: u8,
    pub energy: u8,
    pub health: u8,
    pub sleeping: bool,
    /// Local hour, 0–23.
    pub hour: u32,
    /// Seconds since the previous manual open; `None` if never opened.
    pub secs_since_open: Option<i64>,
}

impl MoodInput {
    pub fn from_pet(pet: &PetState, hour: u32, secs_since_open: Option<i64>) -> Self {
        Self {
            hunger: pet.hunger,
            happiness: pet.happiness,
            energy: pet.energy,
            health: pet.health,
            sleeping: pet.activity == Activity::Sleeping,
            hour,
            secs_since_open,
        }
    }

    fn needs_min(&self) -> u8 {
        self.hunger.min(self.happiness).min(self.energy)
    }
}

/// The mood shown and since when (unix seconds). Saved so the hysteresis of
/// `concerned` and the dwell survive between calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoodRecord {
    pub mood: Mood,
    pub since: i64,
}

/// Why the mood is being decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The popover was just opened: always decided afresh, and the only time
    /// `missed_you` is considered.
    Open,
    /// Re-checked while the popover stays open (or for the tray): changes wait
    /// out MOOD_MIN_DWELL_SECS, except into `concerned`.
    Refresh,
}

/// The R2 table, without the dwell. `was_concerned` keeps `concerned` until
/// everything is back at CONCERNED_EXIT.
pub fn first_match(input: &MoodInput, was_concerned: bool, at_open: bool) -> Mood {
    let needs_min = input.needs_min();
    let concerned = if was_concerned {
        needs_min < CONCERNED_EXIT || input.health < CONCERNED_EXIT
    } else {
        needs_min < CONCERNED_ENTER_NEEDS || input.health < CONCERNED_ENTER_HEALTH
    };
    let sleepy_hours = input.hour >= SLEEPY_START_HOUR || input.hour < SLEEPY_END_HOUR;
    let away_a_day = input.secs_since_open.is_some_and(|secs| secs >= MISSED_YOU_H * 3600);

    if concerned {
        Mood::Concerned
    } else if input.sleeping || input.energy < SLEEPY_ENERGY || sleepy_hours {
        Mood::Sleepy
    } else if at_open && away_a_day {
        Mood::MissedYou
    } else if needs_min < CALM_NEEDS {
        Mood::Calm
    } else {
        Mood::Cheerful
    }
}

/// The mood to show now, given the one shown before.
pub fn decide(input: &MoodInput, prev: Option<&MoodRecord>, now: i64, trigger: Trigger) -> MoodRecord {
    let was_concerned = prev.is_some_and(|p| p.mood == Mood::Concerned);
    let mood = first_match(input, was_concerned, trigger == Trigger::Open);
    let Some(prev) = prev else {
        return MoodRecord { mood, since: now };
    };
    if mood == prev.mood {
        return *prev;
    }
    // A start in the future (the clock was set back) counts as dwelt
    let dwelling = prev.since <= now && now - prev.since < MOOD_MIN_DWELL_SECS;
    let held = trigger == Trigger::Refresh
        && mood != Mood::Concerned
        && prev.mood != Mood::MissedYou
        && dwelling;
    if held {
        *prev
    } else {
        MoodRecord { mood, since: now }
    }
}

/// The need `concerned` is about: the lowest of those below CONCERNED_EXIT,
/// ties in the order hunger, happiness, energy, health. While hysteresis
/// keeps `concerned`, this can move to another need that is now lower.
pub fn concerned_need(input: &MoodInput) -> Option<Need> {
    [
        (Need::Hunger, input.hunger),
        (Need::Happiness, input.happiness),
        (Need::Energy, input.energy),
        (Need::Health, input.health),
    ]
    .into_iter()
    .filter(|&(_, value)| value < CONCERNED_EXIT)
    // min_by_key keeps the first of equal values
    .min_by_key(|&(_, value)| value)
    .map(|(need, _)| need)
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: i64 = 3600;

    /// Noon, all needs 100, opened 1 hour ago.
    fn input() -> MoodInput {
        MoodInput {
            hunger: 100,
            happiness: 100,
            energy: 100,
            health: 100,
            sleeping: false,
            hour: 12,
            secs_since_open: Some(H),
        }
    }

    fn at_open(input: &MoodInput) -> Mood {
        decide(input, None, 1_000, Trigger::Open).mood
    }

    fn record(mood: Mood, since: i64) -> MoodRecord {
        MoodRecord { mood, since }
    }

    // R2 rows one by one

    #[test]
    fn all_well_is_cheerful() {
        assert_eq!(at_open(&input()), Mood::Cheerful);
    }

    #[test]
    fn lowest_need_below_60_is_calm() {
        let i = MoodInput { happiness: 59, ..input() };
        assert_eq!(at_open(&i), Mood::Calm);
        let i = MoodInput { happiness: 60, ..input() };
        assert_eq!(at_open(&i), Mood::Cheerful);
    }

    #[test]
    fn a_day_since_the_last_open_is_missed_you() {
        let i = MoodInput { secs_since_open: Some(24 * H), ..input() };
        assert_eq!(at_open(&i), Mood::MissedYou);
        let i = MoodInput { secs_since_open: Some(24 * H - 1), ..input() };
        assert_eq!(at_open(&i), Mood::Cheerful);
    }

    #[test]
    fn never_opened_is_not_missed_you() {
        let i = MoodInput { secs_since_open: None, ..input() };
        assert_eq!(at_open(&i), Mood::Cheerful);
    }

    #[test]
    fn sleeping_low_energy_or_late_hours_are_sleepy() {
        assert_eq!(at_open(&MoodInput { sleeping: true, ..input() }), Mood::Sleepy);
        assert_eq!(at_open(&MoodInput { energy: 34, ..input() }), Mood::Sleepy);
        // 35 is not sleepy, but still below CALM_NEEDS
        assert_eq!(at_open(&MoodInput { energy: 35, ..input() }), Mood::Calm);
        for hour in [23, 0, 3, 5] {
            assert_eq!(at_open(&MoodInput { hour, ..input() }), Mood::Sleepy, "{}h", hour);
        }
        for hour in [6, 12, 22] {
            assert_eq!(at_open(&MoodInput { hour, ..input() }), Mood::Cheerful, "{}h", hour);
        }
    }

    #[test]
    fn a_need_below_20_or_health_below_30_is_concerned() {
        assert_eq!(at_open(&MoodInput { hunger: 19, ..input() }), Mood::Concerned);
        assert_eq!(at_open(&MoodInput { energy: 19, ..input() }), Mood::Concerned);
        assert_eq!(at_open(&MoodInput { health: 29, ..input() }), Mood::Concerned);
        assert_eq!(at_open(&MoodInput { hunger: 20, ..input() }), Mood::Calm);
        assert_eq!(at_open(&MoodInput { health: 30, ..input() }), Mood::Cheerful);
    }

    // Overlaps: the higher row wins

    #[test]
    fn concerned_beats_sleepy_missed_you_and_calm() {
        let i = MoodInput {
            hunger: 10,
            sleeping: true,
            hour: 2,
            secs_since_open: Some(48 * H),
            ..input()
        };
        assert_eq!(at_open(&i), Mood::Concerned);
    }

    #[test]
    fn sleepy_beats_missed_you_and_calm() {
        let i = MoodInput { hour: 23, secs_since_open: Some(30 * H), happiness: 40, ..input() };
        assert_eq!(at_open(&i), Mood::Sleepy);
    }

    #[test]
    fn missed_you_beats_calm() {
        let i = MoodInput { happiness: 40, secs_since_open: Some(30 * H), ..input() };
        assert_eq!(at_open(&i), Mood::MissedYou);
    }

    #[test]
    fn f2_example_2330_with_needs_70_80_40_is_sleepy() {
        let i = MoodInput { hunger: 70, happiness: 80, energy: 40, hour: 23, ..input() };
        assert_eq!(at_open(&i), Mood::Sleepy);
    }

    #[test]
    fn ac_needs_15_80_80_is_concerned() {
        let i = MoodInput { hunger: 15, happiness: 80, energy: 80, ..input() };
        assert_eq!(at_open(&i), Mood::Concerned);
    }

    // Hysteresis 20 / 30

    #[test]
    fn concerned_holds_until_every_need_is_30() {
        let was = record(Mood::Concerned, 0);
        let i = MoodInput { hunger: 25, ..input() };
        assert_eq!(decide(&i, Some(&was), 10_000, Trigger::Open).mood, Mood::Concerned);
        let i = MoodInput { hunger: 29, ..input() };
        assert_eq!(decide(&i, Some(&was), 10_000, Trigger::Open).mood, Mood::Concerned);
        let i = MoodInput { hunger: 30, ..input() };
        assert_eq!(decide(&i, Some(&was), 10_000, Trigger::Open).mood, Mood::Calm);
    }

    #[test]
    fn needs_between_20_and_30_do_not_start_concerned() {
        let was = record(Mood::Calm, 0);
        let i = MoodInput { hunger: 25, ..input() };
        assert_eq!(decide(&i, Some(&was), 10_000, Trigger::Open).mood, Mood::Calm);
    }

    #[test]
    fn concerned_needs_health_back_at_30_too() {
        let was = record(Mood::Concerned, 0);
        let i = MoodInput { health: 29, ..input() };
        assert_eq!(decide(&i, Some(&was), 10_000, Trigger::Open).mood, Mood::Concerned);
    }

    // Dwell

    #[test]
    fn opening_ignores_the_dwell() {
        let was = record(Mood::Cheerful, 1_000);
        let i = MoodInput { happiness: 40, ..input() };
        let now = decide(&i, Some(&was), 1_001, Trigger::Open);
        assert_eq!(now, record(Mood::Calm, 1_001));
    }

    #[test]
    fn a_refresh_keeps_the_mood_for_600_seconds() {
        let was = record(Mood::Cheerful, 1_000);
        let i = MoodInput { happiness: 40, ..input() };
        assert_eq!(decide(&i, Some(&was), 1_599, Trigger::Refresh), was);
        assert_eq!(decide(&i, Some(&was), 1_600, Trigger::Refresh), record(Mood::Calm, 1_600));
    }

    #[test]
    fn an_unchanged_mood_keeps_its_start() {
        let was = record(Mood::Calm, 1_000);
        let i = MoodInput { happiness: 40, ..input() };
        assert_eq!(decide(&i, Some(&was), 5_000, Trigger::Refresh), was);
        assert_eq!(decide(&i, Some(&was), 5_000, Trigger::Open), was);
    }

    #[test]
    fn concerned_comes_at_once_even_inside_the_dwell() {
        let was = record(Mood::Cheerful, 1_000);
        let i = MoodInput { hunger: 10, ..input() };
        assert_eq!(decide(&i, Some(&was), 1_001, Trigger::Refresh), record(Mood::Concerned, 1_001));
    }

    #[test]
    fn leaving_concerned_waits_out_the_dwell() {
        let was = record(Mood::Concerned, 1_000);
        assert_eq!(decide(&input(), Some(&was), 1_300, Trigger::Refresh), was);
        assert_eq!(decide(&input(), Some(&was), 1_600, Trigger::Refresh).mood, Mood::Cheerful);
    }

    #[test]
    fn flipping_every_3_minutes_for_30_minutes_changes_at_most_3_times() {
        let calm = MoodInput { happiness: 40, ..input() };
        let mut shown = decide(&input(), None, 0, Trigger::Open);
        let mut changes = 0;
        for step in 1..=10 {
            let i = if step % 2 == 1 { calm } else { input() };
            let next = decide(&i, Some(&shown), step * 180, Trigger::Refresh);
            if next.mood != shown.mood {
                changes += 1;
            }
            shown = next;
        }
        assert!(changes <= 3, "{} changes", changes);
    }

    #[test]
    fn a_clock_set_back_does_not_hold_the_mood() {
        let was = record(Mood::Cheerful, 10_000);
        let i = MoodInput { happiness: 40, ..input() };
        assert_eq!(decide(&i, Some(&was), 5_000, Trigger::Refresh), record(Mood::Calm, 5_000));
    }

    #[test]
    fn no_previous_mood_refreshes_to_the_table() {
        let i = MoodInput { happiness: 40, ..input() };
        assert_eq!(decide(&i, None, 50, Trigger::Refresh), record(Mood::Calm, 50));
    }

    // missed_you only at the moment of opening

    #[test]
    fn missed_you_is_not_chosen_on_a_refresh() {
        let i = MoodInput { secs_since_open: Some(30 * H), ..input() };
        assert_eq!(decide(&i, None, 50, Trigger::Refresh).mood, Mood::Cheerful);
    }

    #[test]
    fn missed_you_is_not_held_by_the_dwell() {
        let was = record(Mood::MissedYou, 1_000);
        let i = MoodInput { secs_since_open: Some(30 * H), ..input() };
        assert_eq!(decide(&i, Some(&was), 1_010, Trigger::Refresh), record(Mood::Cheerful, 1_010));
    }

    // The need behind concerned

    #[test]
    fn concerned_need_is_the_lowest_below_30() {
        let i = MoodInput { hunger: 25, happiness: 12, ..input() };
        assert_eq!(concerned_need(&i), Some(Need::Happiness));
        let i = MoodInput { health: 20, energy: 25, ..input() };
        assert_eq!(concerned_need(&i), Some(Need::Health));
        let i = MoodInput { hunger: 15, energy: 15, ..input() };
        assert_eq!(concerned_need(&i), Some(Need::Hunger));
        assert_eq!(concerned_need(&input()), None);
    }

    #[test]
    fn mood_names_and_unknown_values() {
        assert_eq!(serde_json::to_string(&Mood::MissedYou).unwrap(), "\"missed_you\"");
        assert_eq!(Mood::MissedYou.as_str(), "missed_you");
        let back: Mood = serde_json::from_str("\"focused\"").unwrap();
        assert_eq!(back, Mood::Calm);
    }

    #[test]
    fn from_pet_reads_the_needs_and_sleep() {
        let mut pet = PetState::new();
        pet.hunger = 1;
        pet.happiness = 2;
        pet.energy = 3;
        pet.health = 4;
        pet.activity = Activity::Sleeping;
        let i = MoodInput::from_pet(&pet, 7, Some(9));
        assert_eq!(
            i,
            MoodInput { hunger: 1, happiness: 2, energy: 3, health: 4, sleeping: true, hour: 7, secs_since_open: Some(9) }
        );
    }
}

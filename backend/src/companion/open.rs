//! What happens when the popover opens: count the visit, decide the mood,
//! maybe say one line (design/gdd/companion.md R1–R4). Pure: the clock and
//! the dice come in as arguments.

use chrono::{NaiveDateTime, Timelike};
use rand::Rng;
use serde::Serialize;
use crate::pet::PetState;
use super::bond;
use super::consts::*;
use super::lines::{self, Category, Line, LineContext};
use super::mood::{self, Mood, MoodInput, Trigger};
use super::settings;
use super::state::{is_active, CompanionState};

/// One line, as the bubble shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SaidLine {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpenResult {
    /// The pet is a living adult.
    pub enabled: bool,
    pub mood: Mood,
    pub line: Option<SaidLine>,
    /// `STAY WITH YOU?` should be shown.
    pub prompt: bool,
}

/// A manual open of the popover at `now` (unix seconds), `local` being the
/// same moment in local time.
pub fn open<R: Rng + ?Sized>(
    state: &mut CompanionState,
    pet: &PetState,
    now: i64,
    local: NaiveDateTime,
    rng: &mut R,
) -> OpenResult {
    open_with(state, pet, now, local, lines::all(), LINE_SPEAK_P, rng)
}

/// `open` with the lines and the chance of the optional lines given.
pub fn open_with<R: Rng + ?Sized>(
    state: &mut CompanionState,
    pet: &PetState,
    now: i64,
    local: NaiveDateTime,
    lines: &[Line],
    speak_p: f64,
    rng: &mut R,
) -> OpenResult {
    let _ = (state, pet, now, local, lines, speak_p, rng);
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rand::{rngs::StdRng, SeedableRng};
    use crate::companion::state::Onboarding;
    use crate::pet::EvolutionStage;

    const H: i64 = 3600;

    fn at(day: u32, hour: u32, min: u32) -> (i64, NaiveDateTime) {
        let local = NaiveDate::from_ymd_opt(2026, 10, day).unwrap().and_hms_opt(hour, min, 0).unwrap();
        (local.and_utc().timestamp(), local)
    }

    fn adult() -> PetState {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Adult;
        pet.hunger = 100;
        pet.happiness = 100;
        pet.energy = 100;
        pet.health = 100;
        pet
    }

    /// A companion that already said YES, started on Oct 1.
    fn companion(pet: &PetState) -> CompanionState {
        let mut c = CompanionState::default();
        c.advance_day(pet, NaiveDate::from_ymd_opt(2026, 10, 1).unwrap());
        c.onboarding = Onboarding::Yes;
        c
    }

    fn open_at(c: &mut CompanionState, pet: &PetState, when: (i64, NaiveDateTime), p: f64) -> OpenResult {
        open_with(c, pet, when.0, when.1, lines::all(), p, &mut StdRng::seed_from_u64(3))
    }

    fn category_of(result: &OpenResult) -> Option<Category> {
        let id = &result.line.as_ref()?.id;
        lines::all().iter().find(|l| &l.id == id).map(|l| l.category)
    }

    #[test]
    fn a_pet_that_is_not_an_adult_is_disabled_and_untouched() {
        let mut pet = adult();
        pet.stage = EvolutionStage::Child;
        let mut c = CompanionState::default();

        let r = open_at(&mut c, &pet, at(14, 12, 0), 1.0);

        assert!(!r.enabled);
        assert_eq!(r.line, None);
        assert!(!r.prompt);
        assert_eq!(c, CompanionState::default());
    }

    #[test]
    fn a_dead_adult_is_disabled() {
        let mut pet = adult();
        let mut c = companion(&pet);
        pet.is_alive = false;
        let before = c.clone();

        let r = open_at(&mut c, &pet, at(14, 12, 0), 1.0);

        assert!(!r.enabled);
        assert_eq!(r.line, None);
        assert_eq!(c, before);
    }

    #[test]
    fn opening_records_the_visit() {
        let pet = adult();
        let mut c = companion(&pet);
        c.overdue_applied = 2;
        let when = at(14, 12, 0);

        let r = open_at(&mut c, &pet, when, 0.0);

        assert!(r.enabled);
        assert!(c.today.g);
        assert_eq!(c.last_open_at, Some(when.0));
        assert_eq!(c.last_open_date, Some(when.1.date()));
        assert_eq!(c.overdue_applied, 0);
        assert_eq!(c.mood.map(|m| m.mood), Some(r.mood));
    }

    #[test]
    fn the_first_open_of_the_day_greets_once() {
        let pet = adult();
        let mut c = companion(&pet);

        let first = open_at(&mut c, &pet, at(14, 9, 0), 0.0);
        let second = open_at(&mut c, &pet, at(14, 9, 30), 0.0);

        assert_eq!(category_of(&first), Some(Category::GreetFirstToday));
        assert_eq!(second.line, None);
        let next_day = open_at(&mut c, &pet, at(15, 9, 0), 0.0);
        assert_eq!(category_of(&next_day), Some(Category::GreetFirstToday));
    }

    #[test]
    fn the_said_line_is_saved_in_the_history() {
        let pet = adult();
        let mut c = companion(&pet);
        let when = at(14, 9, 0);

        let r = open_at(&mut c, &pet, when, 0.0);

        let id = r.line.unwrap().id;
        assert_eq!(c.lines.recent_ids, vec![id]);
        assert_eq!(c.lines.category_last_at.get("greet_first_today"), Some(&when.0));
    }

    #[test]
    fn ac_back_after_5_hours_then_again_4_hours_later() {
        let pet = adult();
        let mut c = companion(&pet);
        open_at(&mut c, &pet, at(14, 8, 0), 0.0);

        let r = open_at(&mut c, &pet, at(14, 13, 0), 0.0);
        assert_eq!(category_of(&r), Some(Category::ReturnAfterAway));
        let r = open_at(&mut c, &pet, at(14, 17, 0), 0.0);
        assert_eq!(r.line, None);
    }

    #[test]
    fn ac_first_open_after_30_hours_greets_and_is_missed_you() {
        let pet = adult();
        let mut c = companion(&pet);
        open_at(&mut c, &pet, at(13, 8, 0), 0.0);

        let r = open_at(&mut c, &pet, at(14, 14, 0), 0.0);

        assert_eq!(r.mood, Mood::MissedYou);
        assert_eq!(category_of(&r), Some(Category::GreetFirstToday));
        // missed_you is only for the moment of opening
        let later = open_at(&mut c, &pet, at(14, 14, 1), 0.0);
        assert_eq!(later.mood, Mood::Cheerful);
    }

    #[test]
    fn a_hungry_dot_says_so() {
        let mut pet = adult();
        let mut c = companion(&pet);
        open_at(&mut c, &pet, at(14, 8, 0), 0.0);
        pet.hunger = 10;

        let r = open_at(&mut c, &pet, at(14, 9, 0), 0.0);

        assert_eq!(r.mood, Mood::Concerned);
        assert!(r.line.unwrap().id.starts_with("concerned_hunger_"));
        assert!(c.lines.concerned_last_at.contains_key("hunger"));
    }

    #[test]
    fn master_off_keeps_the_mood_but_says_nothing() {
        let pet = adult();
        let mut c = companion(&pet);
        c.settings.master_on = false;

        let r = open_at(&mut c, &pet, at(14, 9, 0), 1.0);

        assert!(r.enabled);
        assert_eq!(r.mood, Mood::Cheerful);
        assert_eq!(r.line, None);
        assert!(c.lines.recent_ids.is_empty());
        assert!(c.today.g);
    }

    #[test]
    fn quiet_says_nothing_until_it_ends() {
        let pet = adult();
        let mut c = companion(&pet);
        let when = at(14, 9, 0);
        c.settings.quiet_until = Some(when.0 + H);

        assert_eq!(open_at(&mut c, &pet, when, 1.0).line, None);
        assert!(open_at(&mut c, &pet, at(14, 10, 0), 1.0).line.is_some());
    }

    #[test]
    fn quiet_hours_do_not_silence_the_popover() {
        let pet = adult();
        let mut c = companion(&pet);
        // 23:00 is inside the default quiet hours 22–08
        let r = open_at(&mut c, &pet, at(14, 23, 0), 0.0);
        assert!(r.line.is_some());
    }

    #[test]
    fn bond_tier_is_marked_once() {
        let pet = adult();
        let mut c = companion(&pet);
        open_at(&mut c, &pet, at(14, 8, 0), 0.0);
        c.bond = 120;

        let r = open_at(&mut c, &pet, at(14, 9, 0), 0.0);
        assert_eq!(category_of(&r), Some(Category::BondTier));
        assert_eq!(c.bond_tier_said, 100);

        for when in [at(14, 10, 0), at(15, 12, 0), at(17, 12, 0)] {
            let r = open_at(&mut c, &pet, when, 0.0);
            assert_ne!(category_of(&r), Some(Category::BondTier));
        }
    }

    #[test]
    fn the_first_adult_open_prompts_without_a_line() {
        let pet = adult();
        let mut c = companion(&pet);
        c.onboarding = Onboarding::Unseen;

        let r = open_at(&mut c, &pet, at(14, 9, 0), 1.0);

        assert!(r.prompt);
        assert_eq!(r.line, None);
        assert!(c.lines.recent_ids.is_empty());
        assert!(matches!(c.onboarding, Onboarding::Later { .. }));
        assert!(!open_at(&mut c, &pet, at(14, 10, 0), 1.0).prompt);
    }

    #[test]
    fn onboarding_waits_while_muted() {
        let pet = adult();
        let mut c = companion(&pet);
        c.onboarding = Onboarding::Unseen;
        c.settings.master_on = false;

        let r = open_at(&mut c, &pet, at(14, 9, 0), 1.0);

        assert!(!r.prompt);
        assert_eq!(c.onboarding, Onboarding::Unseen);
    }

    #[test]
    fn an_adult_never_seen_before_starts_and_prompts() {
        let pet = adult();
        let mut c = CompanionState::default();

        let r = open_at(&mut c, &pet, at(14, 9, 0), 1.0);

        assert!(r.enabled);
        assert!(r.prompt);
        assert_eq!(c.pet_id.as_deref(), Some(pet.id.as_str()));
    }

    #[test]
    fn concerned_is_kept_between_opens_until_needs_recover() {
        let mut pet = adult();
        let mut c = companion(&pet);
        pet.hunger = 15;
        assert_eq!(open_at(&mut c, &pet, at(14, 9, 0), 0.0).mood, Mood::Concerned);
        pet.hunger = 25;
        assert_eq!(open_at(&mut c, &pet, at(14, 9, 5), 0.0).mood, Mood::Concerned);
        pet.hunger = 30;
        assert_eq!(open_at(&mut c, &pet, at(14, 9, 10), 0.0).mood, Mood::Calm);
    }

    #[test]
    fn revive_forgets_mood_and_tier() {
        let pet = adult();
        let mut c = companion(&pet);
        c.bond_tier_said = 300;
        open_at(&mut c, &pet, at(14, 9, 0), 0.0);

        c.reset_for_new_pet();

        assert_eq!(c.mood, None);
        assert_eq!(c.bond_tier_said, 0);
    }

    #[test]
    fn the_same_seed_gives_the_same_open() {
        let pet = adult();
        let run = || {
            let mut c = companion(&pet);
            (9..20)
                .map(|hour| {
                    let (now, local) = at(14, hour, 0);
                    let mut r = StdRng::seed_from_u64(hour as u64);
                    open_with(&mut c, &pet, now, local, lines::all(), 0.5, &mut r).line
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}

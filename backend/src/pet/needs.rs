use super::evolution::EvolutionStage;
use super::state::{PetState, Activity};

// Decay rates per minute (slower = easier game)
const HUNGER_DECAY_RATE: f64 = 0.2;    // was 0.5
const HAPPINESS_DECAY_RATE: f64 = 0.15; // was 0.3
const ENERGY_DECAY_RATE: f64 = 0.1;     // was 0.2

// Sleep modifiers
const SLEEP_HUNGER_DECAY_MULT: f64 = 0.5;  // hunger decays at half rate
const SLEEP_ENERGY_RECOVERY_RATE: f64 = 0.5; // +0.5 energy per minute

const HEALTH_RECOVERY_RATE: f64 = 0.3; // +0.3 health per minute
const HEALTH_DECAY_RATE: f64 = 0.05; // -0.05 health per minute while a need is critical

// Absence ("deep sleep"). See design/gdd/companion.md F5 (Issue #16).
/// A gap longer than this counts as absence: only its first AWAY_THRESHOLD_SECS use the present rules.
pub const AWAY_THRESHOLD_SECS: u64 = 30 * 60;
/// Max elapsed time applied to needs per update. Must stay above the F5 convergence time
/// (AWAY_THRESHOLD + (100 - AWAY_HUNGER_FLOOR) / away hunger rate), so the cap never changes the result.
pub const OFFLINE_CAP_SECS: u64 = 96 * 3600;
/// Max elapsed time added to age per update.
pub const AGE_OFFLINE_CAP_SECS: u64 = 14 * 24 * 3600;
/// Hunger stops falling here while away. If it is already lower when the absence starts, it stays put.
const AWAY_HUNGER_FLOOR: f64 = 30.0;

/// Adult decay multiplier for hunger, happiness and energy, both present and away.
/// See design/gdd/companion.md R5 and F4 (decision 18; safe range 0.15-0.2).
/// Above 0.2, a 9 h/day Adult dies even with FEED 1 + PLAY 1 a day.
pub const ADULT_DECAY_MULT: f64 = 0.2;

/// Decay multiplier for a stage (design F5 `stage_mult`). Recovery and health rules do not use it.
pub fn stage_decay_mult(stage: EvolutionStage) -> f64 {
    match stage {
        EvolutionStage::Adult => ADULT_DECAY_MULT,
        EvolutionStage::Egg | EvolutionStage::Baby | EvolutionStage::Child => 1.0,
    }
}

/// Time since the last update, split into the present part (normal rules) and the away part (deep sleep).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElapsedSplit {
    pub present_secs: u64,
    pub away_secs: u64,
}

/// Splits a gap with no updates (app not running, Mac asleep, lid closed) per F5.
pub fn split_elapsed(elapsed_seconds: u64) -> ElapsedSplit {
    let delta = elapsed_seconds.min(OFFLINE_CAP_SECS);
    if delta > AWAY_THRESHOLD_SECS {
        ElapsedSplit { present_secs: AWAY_THRESHOLD_SECS, away_secs: delta - AWAY_THRESHOLD_SECS }
    } else {
        ElapsedSplit { present_secs: delta, away_secs: 0 }
    }
}

/// Adds absence while the app runs but nobody touches the Mac (F5, P6-b) to the gap split.
/// Idle time past the threshold is the tail of the elapsed time; when both rules apply, the larger away part wins.
pub fn split_elapsed_with_idle(elapsed_seconds: u64, idle_seconds: u64) -> ElapsedSplit {
    let gap = split_elapsed(elapsed_seconds);
    let delta = gap.present_secs + gap.away_secs;
    let idle_away = delta.min(idle_seconds.saturating_sub(AWAY_THRESHOLD_SECS));
    let away_secs = gap.away_secs.max(idle_away);
    ElapsedSplit { present_secs: delta - away_secs, away_secs }
}

/// Needs as real numbers while integrating: the public whole points plus the carried remainders.
#[derive(Debug, Clone, Copy)]
struct Needs {
    hunger: f64,
    happiness: f64,
    energy: f64,
    health: f64,
}

impl Needs {
    fn read(pet: &PetState) -> Self {
        Self {
            hunger: carried(pet.hunger, pet.hunger_frac),
            happiness: carried(pet.happiness, pet.happiness_frac),
            energy: carried(pet.energy, pet.energy_frac),
            health: carried(pet.health, pet.health_frac),
        }
    }

    fn write(&self, pet: &mut PetState) {
        (pet.hunger, pet.hunger_frac) = rounded(self.hunger);
        (pet.happiness, pet.happiness_frac) = rounded(self.happiness);
        (pet.energy, pet.energy_frac) = rounded(self.energy);
        (pet.health, pet.health_frac) = rounded(self.health);
    }
}

/// Clamped, so a hand-edited save or FEED on top of a carried +0.4 cannot leave 0..=100.
fn carried(whole: u8, frac: f64) -> f64 {
    let frac = if frac.is_finite() { frac } else { 0.0 };
    (f64::from(whole) + frac).clamp(0.0, 100.0)
}

/// Rounds to the nearest whole point (98.5 shows as 99) and returns what is left over.
/// Float noise is snapped away first, so 98.4999999999 rounds like 98.5.
fn rounded(value: f64) -> (u8, f64) {
    let value = value.clamp(0.0, 100.0);
    let whole = ((value * 1e9).round() / 1e9).round();
    (whole as u8, value - whole)
}

/// Whole minutes, then the remaining part of a minute (F0-3: long gaps are integrated one minute at a time).
fn minute_steps(seconds: u64) -> impl Iterator<Item = f64> {
    let last_partial_minute = (seconds % 60) as f64 / 60.0;
    (0..seconds / 60)
        .map(|_| 1.0)
        .chain((last_partial_minute > 0.0).then_some(last_partial_minute))
}

fn allows_health_recovery(hunger: f64, happiness: f64, energy: f64) -> bool {
    hunger > 50.0 && happiness > 50.0 && energy > 30.0
}

/// One step of the present rules. Health reacts to the needs as they are at this step.
/// `stage_mult` scales the decays only; sleep's energy recovery stays at its rate (design F5).
fn present_step(needs: &mut Needs, minutes: f64, sleeping: bool, stage_mult: f64) {
    // Hunger decays slower when sleeping
    let hunger_rate = if sleeping { HUNGER_DECAY_RATE * SLEEP_HUNGER_DECAY_MULT } else { HUNGER_DECAY_RATE };
    needs.hunger = (needs.hunger - hunger_rate * stage_mult * minutes).max(0.0);

    // Happiness: no decay while sleeping. Energy: recovers while sleeping, decays otherwise
    if sleeping {
        needs.energy = (needs.energy + SLEEP_ENERGY_RECOVERY_RATE * minutes).min(100.0);
    } else {
        needs.happiness = (needs.happiness - HAPPINESS_DECAY_RATE * stage_mult * minutes).max(0.0);
        needs.energy = (needs.energy - ENERGY_DECAY_RATE * stage_mult * minutes).max(0.0);
    }

    // Health is affected by critical needs (more forgiving thresholds)
    if needs.hunger < 10.0 || needs.happiness < 10.0 || needs.energy < 5.0 {
        needs.health = (needs.health - HEALTH_DECAY_RATE * minutes).max(0.0);
    } else if allows_health_recovery(needs.hunger, needs.happiness, needs.energy) {
        needs.health = (needs.health + HEALTH_RECOVERY_RATE * minutes).min(100.0);
    }
}

/// The present rules for `elapsed_seconds` starting at `pet.last_update`, one minute at a time.
/// A sleep that ends inside the span is split there (sleeping rules before, awake rules after).
/// Death is checked after every step, and decay stops once the pet dies.
/// `stage_mult` comes from `stage_decay_mult`, taken by the caller once for the whole span.
pub fn apply_decay(pet: &mut PetState, elapsed_seconds: u64, stage_mult: f64) {
    let sleeping_secs = match (pet.activity, pet.activity_until) {
        (Activity::Sleeping, Some(until)) => (until - pet.last_update).max(0) as u64,
        (Activity::Sleeping, None) => u64::MAX,
        _ => 0,
    }
    .min(elapsed_seconds);

    integrate_present(pet, sleeping_secs, true, stage_mult);
    integrate_present(pet, elapsed_seconds - sleeping_secs, false, stage_mult);
}

fn integrate_present(pet: &mut PetState, seconds: u64, sleeping: bool, stage_mult: f64) {
    let mut needs = Needs::read(pet);
    for minutes in minute_steps(seconds) {
        if !pet.is_alive {
            return;
        }
        present_step(&mut needs, minutes, sleeping, stage_mult);
        needs.write(pet);
        super::lifecycle::check_death(pet);
    }
}

/// Deep sleep for the away part (F5): the sleep rules, plus a hunger floor and no health loss.
/// Never kills the pet and never changes `activity`, so FEED is not refused as "sleeping" on return.
pub fn apply_deep_sleep(pet: &mut PetState, away_seconds: u64, stage_mult: f64) {
    if !pet.is_alive || away_seconds == 0 {
        return;
    }

    let hunger_rate = HUNGER_DECAY_RATE * SLEEP_HUNGER_DECAY_MULT * stage_mult;
    let mut needs = Needs::read(pet);
    let hunger_floor = AWAY_HUNGER_FLOOR.min(needs.hunger);

    for minutes in minute_steps(away_seconds) {
        // The floor is at most 30, so hunger there rules out recovery and nothing changes from here on
        if needs.hunger <= hunger_floor && needs.energy >= 100.0 {
            break;
        }
        needs.hunger = (needs.hunger - hunger_rate * minutes).max(hunger_floor);
        needs.energy = (needs.energy + SLEEP_ENERGY_RECOVERY_RATE * minutes).min(100.0);
        // happiness: no decay while sleeping
        if allows_health_recovery(needs.hunger, needs.happiness, needs.energy) {
            needs.health = (needs.health + HEALTH_RECOVERY_RATE * minutes).min(100.0);
        }
    }

    needs.write(pet);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_pet() -> PetState {
        PetState::new()
    }

    #[test]
    fn test_no_decay_at_zero_elapsed() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 0, 1.0);
        assert_eq!(pet.hunger, 100);
        assert_eq!(pet.happiness, 100);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_decay_after_one_minute() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 60, 1.0); // 1 minute
        // 0.2 * 1 = 0.2 -> truncated to 0 for u8
        // Decay is very small at 1 minute, so stats stay at 100
        assert!(pet.hunger >= 99);
        assert!(pet.happiness >= 99);
        assert!(pet.energy >= 99);
    }

    #[test]
    fn test_decay_after_ten_minutes() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 600, 1.0); // 10 minutes
        // hunger: 0.2 * 10 = 2
        // happiness: 0.15 * 10 = 1
        // energy: 0.1 * 10 = 1
        assert_eq!(pet.hunger, 98);
        assert_eq!(pet.happiness, 99);
        assert_eq!(pet.energy, 99);
    }

    #[test]
    fn test_decay_after_one_hour() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 3600, 1.0); // 60 minutes
        // hunger: 0.2 * 60 = 12
        // happiness: 0.15 * 60 = 9
        // energy: 0.1 * 60 = 6
        assert_eq!(pet.hunger, 88);
        assert_eq!(pet.happiness, 91);
        assert_eq!(pet.energy, 94);
    }

    #[test]
    fn test_stats_dont_go_below_zero() {
        let mut pet = new_pet();
        pet.hunger = 5;
        pet.happiness = 5;
        pet.energy = 5;
        apply_decay(&mut pet, 36000, 1.0); // 600 minutes - massive decay
        assert_eq!(pet.hunger, 0);
        assert_eq!(pet.happiness, 0);
        assert_eq!(pet.energy, 0);
    }

    #[test]
    fn test_health_decays_when_hunger_critical() {
        let mut pet = new_pet();
        pet.hunger = 5; // critical
        pet.happiness = 50;
        pet.energy = 50;
        pet.health = 100;
        apply_decay(&mut pet, 6000, 1.0); // 100 minutes
        // health_decay = 0.05 * 100 = 5
        assert!(pet.health < 100);
    }

    #[test]
    fn test_health_recovers_when_stats_good() {
        let mut pet = new_pet();
        pet.hunger = 80;
        pet.happiness = 80;
        pet.energy = 80;
        pet.health = 50;
        apply_decay(&mut pet, 600, 1.0); // 10 minutes
        // recovery = 0.3 * 10 = 3
        assert!(pet.health > 50);
    }

    #[test]
    fn test_health_capped_at_100() {
        let mut pet = new_pet();
        pet.hunger = 80;
        pet.happiness = 80;
        pet.energy = 80;
        pet.health = 99;
        apply_decay(&mut pet, 600, 1.0);
        assert!(pet.health <= 100);
    }

    #[test]
    fn test_no_decay_when_dead() {
        let mut pet = new_pet();
        pet.is_alive = false;
        pet.hunger = 50;
        apply_decay(&mut pet, 3600, 1.0);
        assert_eq!(pet.hunger, 50); // unchanged
    }

    #[test]
    fn test_sleeping_energy_recovers() {
        let mut pet = new_pet();
        pet.energy = 50;
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 1200, 1.0); // 20 minutes
        // recovery = 0.5 * 20 = 10
        assert_eq!(pet.energy, 60);
    }

    #[test]
    fn test_sleeping_energy_capped_at_100() {
        let mut pet = new_pet();
        pet.energy = 95;
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 3600, 1.0); // 60 minutes -> +30 recovery
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_sleeping_hunger_decays_slower() {
        let mut pet = new_pet();
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 3600, 1.0); // 60 minutes
        // hunger: 0.2 * 0.5 * 60 = 6 (vs 12 when awake)
        assert_eq!(pet.hunger, 94);
    }

    #[test]
    fn test_sleeping_happiness_no_decay() {
        let mut pet = new_pet();
        pet.activity = Activity::Sleeping;
        pet.happiness = 80;
        apply_decay(&mut pet, 3600, 1.0); // 60 minutes
        assert_eq!(pet.happiness, 80); // unchanged
    }

    const HOUR: u64 = 3600;

    #[test]
    fn test_split_short_gap_is_all_present() {
        assert_eq!(split_elapsed(600), ElapsedSplit { present_secs: 600, away_secs: 0 });
        // Exactly the threshold is not yet absence
        assert_eq!(split_elapsed(AWAY_THRESHOLD_SECS), ElapsedSplit { present_secs: AWAY_THRESHOLD_SECS, away_secs: 0 });
    }

    #[test]
    fn test_split_long_gap_keeps_threshold_present() {
        assert_eq!(
            split_elapsed(63 * HOUR),
            ElapsedSplit { present_secs: AWAY_THRESHOLD_SECS, away_secs: 63 * HOUR - AWAY_THRESHOLD_SECS }
        );
    }

    #[test]
    fn test_split_clamps_to_offline_cap() {
        assert_eq!(split_elapsed(200 * HOUR), split_elapsed(OFFLINE_CAP_SECS));
        assert_eq!(split_elapsed(200 * HOUR).away_secs, 96 * HOUR - AWAY_THRESHOLD_SECS);
    }

    #[test]
    fn test_split_short_idle_is_all_present() {
        assert_eq!(split_elapsed_with_idle(5, 0), ElapsedSplit { present_secs: 5, away_secs: 0 });
        assert_eq!(split_elapsed_with_idle(5, AWAY_THRESHOLD_SECS), ElapsedSplit { present_secs: 5, away_secs: 0 });
    }

    #[test]
    fn test_split_idle_past_threshold_is_away() {
        // Idle 40 minutes: the last 10 minutes are away, so a 5 second update is all away
        assert_eq!(split_elapsed_with_idle(5, 40 * 60), ElapsedSplit { present_secs: 0, away_secs: 5 });
        // The update that crosses the threshold is split there: present first, away after
        assert_eq!(
            split_elapsed_with_idle(60, AWAY_THRESHOLD_SECS + 20),
            ElapsedSplit { present_secs: 40, away_secs: 20 }
        );
    }

    #[test]
    fn test_split_takes_larger_away_of_gap_and_idle() {
        // Gap rule wins: 2 h gap with fresh input
        assert_eq!(split_elapsed_with_idle(2 * HOUR, 0), split_elapsed(2 * HOUR));
        // Idle rule wins: 2 h gap after 3 h without input
        assert_eq!(split_elapsed_with_idle(2 * HOUR, 3 * HOUR), ElapsedSplit { present_secs: 0, away_secs: 2 * HOUR });
        // Both are clamped to the cap
        assert_eq!(split_elapsed_with_idle(200 * HOUR, 300 * HOUR).away_secs, OFFLINE_CAP_SECS);
    }

    #[test]
    fn test_decay_carries_fractions_between_calls() {
        let mut pet = new_pet();
        for _ in 0..12 {
            apply_decay(&mut pet, 5, 1.0);
            pet.last_update += 5;
        }
        let mut once = new_pet();
        apply_decay(&mut once, 60, 1.0);
        assert!((pet.hunger_frac - once.hunger_frac).abs() < 1e-9);
        assert!((f64::from(pet.hunger) + pet.hunger_frac - 99.8).abs() < 1e-9);
    }

    #[test]
    fn test_decay_stops_at_death() {
        let mut pet = new_pet();
        pet.hunger = 0;
        pet.happiness = 0;
        pet.energy = 1;
        pet.health = 80;
        apply_decay(&mut pet, 3600, 1.0);
        assert!(!pet.is_alive);
        // Died within the first 5 minutes (energy below 0.5), so health barely moved
        assert!(pet.health >= 79, "health {}", pet.health);
    }

    #[test]
    fn test_deep_sleep_hunger_stops_at_floor() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        assert_eq!(pet.hunger, 30);
    }

    #[test]
    fn test_deep_sleep_hunger_decays_at_sleep_rate() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, HOUR, 1.0);
        // 0.2 * 0.5 * 60 = 6, the same as the existing sleep rule
        assert_eq!(pet.hunger, 94);
    }

    #[test]
    fn test_deep_sleep_hunger_below_floor_stays() {
        let mut pet = new_pet();
        pet.hunger = 12;
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        assert_eq!(pet.hunger, 12);
    }

    #[test]
    fn test_deep_sleep_keeps_happiness_and_recovers_energy() {
        let mut pet = new_pet();
        pet.happiness = 40;
        pet.energy = 10;
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        assert_eq!(pet.happiness, 40);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_deep_sleep_never_reduces_health() {
        let mut pet = new_pet();
        pet.hunger = 0;
        pet.happiness = 0;
        pet.energy = 0;
        pet.health = 1;
        apply_deep_sleep(&mut pet, 96 * HOUR, 1.0);
        assert_eq!(pet.health, 1);
        assert!(pet.is_alive);
    }

    #[test]
    fn test_deep_sleep_recovers_health_while_needs_are_good() {
        let mut pet = new_pet();
        pet.health = 50;
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        // hunger stays above 50 for 500 minutes, far longer than the ~167 minutes needed to reach 100
        assert_eq!(pet.health, 100);
    }

    #[test]
    fn test_deep_sleep_does_not_touch_activity() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        assert_eq!(pet.activity, Activity::Idle);
    }

    #[test]
    fn test_stage_decay_mult_is_reduced_only_for_adult() {
        assert_eq!(stage_decay_mult(EvolutionStage::Egg), 1.0);
        assert_eq!(stage_decay_mult(EvolutionStage::Baby), 1.0);
        assert_eq!(stage_decay_mult(EvolutionStage::Child), 1.0);
        assert_eq!(stage_decay_mult(EvolutionStage::Adult), ADULT_DECAY_MULT);
        assert_eq!(ADULT_DECAY_MULT, 0.2);
    }

    #[test]
    fn test_adult_present_decay() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 600 * 60, ADULT_DECAY_MULT);
        // 600 minutes at 0.04 / 0.03 / 0.02
        assert_eq!([pet.hunger, pet.happiness, pet.energy], [76, 82, 88]);
    }

    #[test]
    fn test_adult_sleeping_decays_hunger_slower_but_recovers_energy_at_full_rate() {
        let mut pet = new_pet();
        pet.energy = 50;
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, HOUR, ADULT_DECAY_MULT);
        // hunger 0.2 * 0.5 * 0.2 * 60 = 1.2; energy recovery is not a decay: +0.5 * 60
        assert_eq!(pet.hunger, 99);
        assert_eq!(pet.energy, 80);
    }

    #[test]
    fn test_adult_health_rules_unchanged() {
        let mut pet = new_pet();
        pet.hunger = 5;
        pet.happiness = 50;
        pet.energy = 50;
        apply_decay(&mut pet, 100 * 60, ADULT_DECAY_MULT);
        // health -0.05 per minute while hunger is critical, as for every stage
        assert_eq!(pet.health, 95);
    }

    #[test]
    fn test_adult_deep_sleep_hunger_decays_at_a_fifth() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, 10 * HOUR, ADULT_DECAY_MULT);
        // 0.2 * 0.5 * 0.2 * 600 = 12
        assert_eq!(pet.hunger, 88);
        // Still stops at the floor
        apply_deep_sleep(&mut pet, 96 * HOUR, ADULT_DECAY_MULT);
        assert_eq!(pet.hunger, 30);
    }

    #[test]
    fn test_deep_sleep_noop_when_dead() {
        let mut pet = new_pet();
        pet.is_alive = false;
        apply_deep_sleep(&mut pet, 24 * HOUR, 1.0);
        assert_eq!(pet.hunger, 100);
    }
}

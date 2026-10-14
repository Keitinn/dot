use super::state::{PetState, Activity};

// Decay rates per minute (slower = easier game)
const HUNGER_DECAY_RATE: f64 = 0.2;    // was 0.5
const HAPPINESS_DECAY_RATE: f64 = 0.15; // was 0.3
const ENERGY_DECAY_RATE: f64 = 0.1;     // was 0.2

// Sleep modifiers
const SLEEP_HUNGER_DECAY_MULT: f64 = 0.5;  // hunger decays at half rate
const SLEEP_ENERGY_RECOVERY_RATE: f64 = 0.5; // +0.5 energy per minute

const HEALTH_RECOVERY_RATE: f64 = 0.3; // +0.3 health per minute

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
/// Stage multiplier for away hunger decay. 1.0 for every stage until the Companion MVP adds Adult 0.2 (R5).
const AWAY_STAGE_DECAY_MULT: f64 = 1.0;

/// Time since the last update, split into the present part (normal rules) and the away part (deep sleep).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElapsedSplit {
    pub present_secs: u64,
    pub away_secs: u64,
}

/// Splits a gap with no updates (app not running, Mac asleep, lid closed) per F5.
/// Absence detected from input idle time (P6-b) is meant to be combined here by taking the larger away part.
pub fn split_elapsed(elapsed_seconds: u64) -> ElapsedSplit {
    let delta = elapsed_seconds.min(OFFLINE_CAP_SECS);
    if delta > AWAY_THRESHOLD_SECS {
        ElapsedSplit { present_secs: AWAY_THRESHOLD_SECS, away_secs: delta - AWAY_THRESHOLD_SECS }
    } else {
        ElapsedSplit { present_secs: delta, away_secs: 0 }
    }
}

fn allows_health_recovery(hunger: f64, happiness: f64, energy: f64) -> bool {
    hunger > 50.0 && happiness > 50.0 && energy > 30.0
}

pub fn apply_decay(pet: &mut PetState, elapsed_seconds: u64) {
    if !pet.is_alive {
        return;
    }

    let minutes = elapsed_seconds as f64 / 60.0;
    let is_sleeping = pet.activity == Activity::Sleeping;

    // Apply hunger decay (slower when sleeping)
    let hunger_rate = if is_sleeping { HUNGER_DECAY_RATE * SLEEP_HUNGER_DECAY_MULT } else { HUNGER_DECAY_RATE };
    let hunger_decay = (hunger_rate * minutes) as u8;
    pet.hunger = pet.hunger.saturating_sub(hunger_decay);

    // Happiness: no decay while sleeping
    if !is_sleeping {
        let happiness_decay = (HAPPINESS_DECAY_RATE * minutes) as u8;
        pet.happiness = pet.happiness.saturating_sub(happiness_decay);
    }

    // Energy: recovers while sleeping, decays otherwise
    if is_sleeping {
        let energy_recovery = (SLEEP_ENERGY_RECOVERY_RATE * minutes) as u8;
        pet.energy = (pet.energy + energy_recovery).min(100);
    } else {
        let energy_decay = (ENERGY_DECAY_RATE * minutes) as u8;
        pet.energy = pet.energy.saturating_sub(energy_decay);
    }

    // Health is affected by critical needs (more forgiving thresholds)
    if pet.hunger < 10 || pet.happiness < 10 || pet.energy < 5 {
        let health_decay = (0.05 * minutes) as u8; // slower health decay
        pet.health = pet.health.saturating_sub(health_decay);
    } else if allows_health_recovery(f64::from(pet.hunger), f64::from(pet.happiness), f64::from(pet.energy)) {
        // Recover health more easily
        let health_recovery = (HEALTH_RECOVERY_RATE * minutes) as u8; // faster recovery
        pet.health = (pet.health + health_recovery).min(100);
    }
}

/// Deep sleep for the away part of a gap (F5): the sleep rules, plus a hunger floor and no health loss.
/// Steps minute by minute in f64 so slow rates are not truncated away, then rounds back to u8.
/// Never kills the pet and never changes `activity`, so FEED is not refused as "sleeping" on return.
pub fn apply_deep_sleep(pet: &mut PetState, away_seconds: u64) {
    if !pet.is_alive || away_seconds == 0 {
        return;
    }

    let hunger_rate = HUNGER_DECAY_RATE * SLEEP_HUNGER_DECAY_MULT * AWAY_STAGE_DECAY_MULT;
    let hunger_floor = AWAY_HUNGER_FLOOR.min(f64::from(pet.hunger));
    let happiness = f64::from(pet.happiness); // no decay while sleeping
    let mut hunger = f64::from(pet.hunger);
    let mut energy = f64::from(pet.energy);
    let mut health = f64::from(pet.health);

    let whole_minutes = away_seconds / 60;
    let last_partial_minute = (away_seconds % 60) as f64 / 60.0;
    let steps = (0..whole_minutes)
        .map(|_| 1.0)
        .chain((last_partial_minute > 0.0).then_some(last_partial_minute));

    for minutes in steps {
        // The floor is at most 30, so hunger there rules out recovery and nothing changes from here on
        if hunger <= hunger_floor && energy >= 100.0 {
            break;
        }
        hunger = (hunger - hunger_rate * minutes).max(hunger_floor);
        energy = (energy + SLEEP_ENERGY_RECOVERY_RATE * minutes).min(100.0);
        if allows_health_recovery(hunger, happiness, energy) {
            health = (health + HEALTH_RECOVERY_RATE * minutes).min(100.0);
        }
    }

    pet.hunger = hunger.round() as u8;
    pet.energy = energy.round() as u8;
    pet.health = health.round() as u8;
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
        apply_decay(&mut pet, 0);
        assert_eq!(pet.hunger, 100);
        assert_eq!(pet.happiness, 100);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_decay_after_one_minute() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 60); // 1 minute
        // 0.2 * 1 = 0.2 -> truncated to 0 for u8
        // Decay is very small at 1 minute, so stats stay at 100
        assert!(pet.hunger >= 99);
        assert!(pet.happiness >= 99);
        assert!(pet.energy >= 99);
    }

    #[test]
    fn test_decay_after_ten_minutes() {
        let mut pet = new_pet();
        apply_decay(&mut pet, 600); // 10 minutes
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
        apply_decay(&mut pet, 3600); // 60 minutes
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
        apply_decay(&mut pet, 36000); // 600 minutes - massive decay
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
        apply_decay(&mut pet, 6000); // 100 minutes
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
        apply_decay(&mut pet, 600); // 10 minutes
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
        apply_decay(&mut pet, 600);
        assert!(pet.health <= 100);
    }

    #[test]
    fn test_no_decay_when_dead() {
        let mut pet = new_pet();
        pet.is_alive = false;
        pet.hunger = 50;
        apply_decay(&mut pet, 3600);
        assert_eq!(pet.hunger, 50); // unchanged
    }

    #[test]
    fn test_sleeping_energy_recovers() {
        let mut pet = new_pet();
        pet.energy = 50;
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 1200); // 20 minutes
        // recovery = 0.5 * 20 = 10
        assert_eq!(pet.energy, 60);
    }

    #[test]
    fn test_sleeping_energy_capped_at_100() {
        let mut pet = new_pet();
        pet.energy = 95;
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 3600); // 60 minutes -> +30 recovery
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_sleeping_hunger_decays_slower() {
        let mut pet = new_pet();
        pet.activity = Activity::Sleeping;
        apply_decay(&mut pet, 3600); // 60 minutes
        // hunger: 0.2 * 0.5 * 60 = 6 (vs 12 when awake)
        assert_eq!(pet.hunger, 94);
    }

    #[test]
    fn test_sleeping_happiness_no_decay() {
        let mut pet = new_pet();
        pet.activity = Activity::Sleeping;
        pet.happiness = 80;
        apply_decay(&mut pet, 3600); // 60 minutes
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
    fn test_deep_sleep_hunger_stops_at_floor() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, 24 * HOUR);
        assert_eq!(pet.hunger, 30);
    }

    #[test]
    fn test_deep_sleep_hunger_decays_at_sleep_rate() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, HOUR);
        // 0.2 * 0.5 * 60 = 6, the same as the existing sleep rule
        assert_eq!(pet.hunger, 94);
    }

    #[test]
    fn test_deep_sleep_hunger_below_floor_stays() {
        let mut pet = new_pet();
        pet.hunger = 12;
        apply_deep_sleep(&mut pet, 24 * HOUR);
        assert_eq!(pet.hunger, 12);
    }

    #[test]
    fn test_deep_sleep_keeps_happiness_and_recovers_energy() {
        let mut pet = new_pet();
        pet.happiness = 40;
        pet.energy = 10;
        apply_deep_sleep(&mut pet, 24 * HOUR);
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
        apply_deep_sleep(&mut pet, 96 * HOUR);
        assert_eq!(pet.health, 1);
        assert!(pet.is_alive);
    }

    #[test]
    fn test_deep_sleep_recovers_health_while_needs_are_good() {
        let mut pet = new_pet();
        pet.health = 50;
        apply_deep_sleep(&mut pet, 24 * HOUR);
        // hunger stays above 50 for 500 minutes, far longer than the ~167 minutes needed to reach 100
        assert_eq!(pet.health, 100);
    }

    #[test]
    fn test_deep_sleep_does_not_touch_activity() {
        let mut pet = new_pet();
        apply_deep_sleep(&mut pet, 24 * HOUR);
        assert_eq!(pet.activity, Activity::Idle);
    }

    #[test]
    fn test_deep_sleep_noop_when_dead() {
        let mut pet = new_pet();
        pet.is_alive = false;
        apply_deep_sleep(&mut pet, 24 * HOUR);
        assert_eq!(pet.hunger, 100);
    }
}

use serde::{Deserialize, Serialize};
use chrono::Utc;
use super::evolution::EvolutionStage;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Idle,
    Sleeping,
    Eating,
    Playing,
}

impl Default for Activity {
    fn default() -> Self {
        Activity::Idle
    }
}

pub const FEED_COOLDOWN_SECS: i64 = 1800; // 30 minutes
pub const PLAY_COOLDOWN_SECS: i64 = 1200; // 20 minutes
const EATING_DURATION_SECS: i64 = 30;
const PLAYING_DURATION_SECS: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PetState {
    pub id: String,
    pub name: String,
    pub stage: EvolutionStage,
    pub age: u64,           // in seconds
    pub hunger: u8,         // 0-100
    pub happiness: u8,      // 0-100
    pub energy: u8,         // 0-100
    pub health: u8,         // 0-100
    pub is_alive: bool,
    pub birth_time: i64,
    pub last_update: i64,
    #[serde(default)]
    pub activity: Activity,
    #[serde(default)]
    pub activity_until: Option<i64>,
    #[serde(default)]
    pub last_feed_time: Option<i64>,
    #[serde(default)]
    pub last_play_time: Option<i64>,
    // What rounding hunger..health to whole points left over, in [-0.5, 0.5].
    // Carried to the next update so 5-second polls add up (design F0, P2). Older saves lack them: 0.
    #[serde(default)]
    pub(crate) hunger_frac: f64,
    #[serde(default)]
    pub(crate) happiness_frac: f64,
    #[serde(default)]
    pub(crate) energy_frac: f64,
    #[serde(default)]
    pub(crate) health_frac: f64,
}

impl PetState {
    pub fn new() -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: "dot".to_string(),
            stage: EvolutionStage::Egg,
            age: 0,
            hunger: 100,
            happiness: 100,
            energy: 100,
            health: 100,
            is_alive: true,
            birth_time: now,
            last_update: now,
            activity: Activity::Idle,
            activity_until: None,
            last_feed_time: None,
            last_play_time: None,
            hunger_frac: 0.0,
            happiness_frac: 0.0,
            energy_frac: 0.0,
            health_frac: 0.0,
        }
    }

    pub fn update(&mut self) {
        self.update_at(Utc::now().timestamp(), crate::signals::idle_seconds());
    }

    /// `idle_secs` is how long the user has gone without input (F5). Taken as an argument so tests can set it.
    fn update_at(&mut self, now: i64, idle_secs: u64) {
        // The stage at the start of the span decides the whole span, even if the pet evolves during it
        let stage_mult = super::needs::stage_decay_mult(self.stage);
        self.update_at_with_mult(now, idle_secs, stage_mult);
    }

    /// `update_at` with the stage decay multiplier given, so tests can pin it.
    fn update_at_with_mult(&mut self, now: i64, idle_secs: u64, stage_mult: f64) {
        if !self.is_alive {
            return;
        }

        // A clock moved backwards counts as no time passing
        let elapsed = (now - self.last_update).max(0) as u64;

        // Update age (keeps growing while away, up to a cap per gap)
        self.age += elapsed.min(super::needs::AGE_OFFLINE_CAP_SECS);

        // Update needs: the present part first, then the away part as deep sleep.
        // The present part needs the activity as it was, to split at the end of a sleep
        let split = super::needs::split_elapsed_with_idle(elapsed, idle_secs);
        super::needs::apply_decay(self, split.present_secs, stage_mult);

        // Check if current activity has ended
        if let Some(until) = self.activity_until {
            if now >= until {
                self.activity = Activity::Idle;
                self.activity_until = None;
            }
        }

        // Check evolution
        super::evolution::check_evolution(self);

        // Check death conditions. Deep sleep cannot kill, so only the present part is checked
        super::lifecycle::check_death(self);

        super::needs::apply_deep_sleep(self, split.away_secs, stage_mult);

        self.last_update = now;
    }

    pub fn feed(&mut self) {
        self.update();
        self.apply_feed(Utc::now().timestamp());
    }

    /// FEED on a pet already updated to `now`.
    fn apply_feed(&mut self, now: i64) {
        if !self.is_alive {
            return;
        }

        // Can't feed while sleeping
        if self.activity == Activity::Sleeping {
            return;
        }

        self.hunger = (self.hunger + 30).min(100);
        self.happiness = (self.happiness + 5).min(100);
        self.last_feed_time = Some(now);
        self.activity = Activity::Eating;
        self.activity_until = Some(now + EATING_DURATION_SECS);
    }

    pub fn play(&mut self) {
        self.update();
        self.apply_play(Utc::now().timestamp());
    }

    /// PLAY on a pet already updated to `now`.
    fn apply_play(&mut self, now: i64) {
        if !self.is_alive {
            return;
        }

        // Can't play while sleeping
        if self.activity == Activity::Sleeping {
            return;
        }

        self.happiness = (self.happiness + 20).min(100);
        self.energy = self.energy.saturating_sub(10);
        self.hunger = self.hunger.saturating_sub(5);
        self.last_play_time = Some(now);
        self.activity = Activity::Playing;
        self.activity_until = Some(now + PLAYING_DURATION_SECS);
    }

    /// Apply a purchased snack. Does not touch the FEED cooldown.
    /// Errors (without changing anything) if the pet is dead, asleep, or the snack would have no effect.
    pub fn eat_snack(&mut self, snack: &super::economy::Snack) -> Result<(), String> {
        self.update();
        if !self.is_alive {
            return Err("dot is gone".to_string());
        }
        if self.activity == Activity::Sleeping {
            return Err("dot is sleeping".to_string());
        }

        let hunger = self.hunger.saturating_add(snack.hunger).min(100);
        let happiness = self.happiness.saturating_add(snack.happiness).min(100);
        let energy = self.energy.saturating_add(snack.energy).min(100);
        let health = self.health.saturating_add(snack.health).min(100);
        if hunger == self.hunger && happiness == self.happiness && energy == self.energy && health == self.health {
            return Err("dot doesn't need that".to_string());
        }

        let now = Utc::now().timestamp();
        self.hunger = hunger;
        self.happiness = happiness;
        self.energy = energy;
        self.health = health;
        self.activity = Activity::Eating;
        self.activity_until = Some(now + EATING_DURATION_SECS);
        Ok(())
    }

    pub fn sleep(&mut self) {
        self.update();
        if !self.is_alive {
            return;
        }

        // Already sleeping — no-op
        if self.activity == Activity::Sleeping {
            return;
        }

        let now = Utc::now().timestamp();
        // Sleep for 1-4 hours (random)
        let sleep_hours = rand::random_range(1..=4);
        let sleep_secs = sleep_hours * 3600;
        self.activity = Activity::Sleeping;
        self.activity_until = Some(now + sleep_secs as i64);
    }

    /// Computed status string describing what dot is doing
    pub fn status(&self) -> String {
        match self.activity {
            Activity::Sleeping => {
                if let Some(until) = self.activity_until {
                    let now = Utc::now().timestamp();
                    let remaining = (until - now).max(0);
                    let hours = remaining / 3600;
                    let minutes = (remaining % 3600) / 60;
                    if hours > 0 {
                        format!("sleeping ({}h {}m left)", hours, minutes)
                    } else {
                        format!("sleeping ({}m left)", minutes)
                    }
                } else {
                    "sleeping".to_string()
                }
            }
            Activity::Eating => "eating".to_string(),
            Activity::Playing => "playing".to_string(),
            Activity::Idle => {
                if self.hunger < 20 {
                    "hungry".to_string()
                } else if self.energy < 20 {
                    "tired".to_string()
                } else if self.happiness < 20 {
                    "sad".to_string()
                } else {
                    "content".to_string()
                }
            }
        }
    }
}

impl Default for PetState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_pet_defaults() {
        let pet = PetState::new();
        assert_eq!(pet.name, "dot");
        assert_eq!(pet.stage, EvolutionStage::Egg);
        assert_eq!(pet.age, 0);
        assert_eq!(pet.hunger, 100);
        assert_eq!(pet.happiness, 100);
        assert_eq!(pet.energy, 100);
        assert_eq!(pet.health, 100);
        assert!(pet.is_alive);
        assert_eq!(pet.activity, Activity::Idle);
        assert!(pet.activity_until.is_none());
        assert!(pet.last_feed_time.is_none());
        assert!(pet.last_play_time.is_none());
    }

    #[test]
    fn test_feed_increases_hunger() {
        let mut pet = PetState::new();
        pet.hunger = 50;
        pet.last_update = Utc::now().timestamp(); // prevent decay
        pet.feed();
        assert!(pet.hunger > 50);
    }

    #[test]
    fn test_feed_caps_at_100() {
        let mut pet = PetState::new();
        pet.hunger = 90;
        pet.last_update = Utc::now().timestamp();
        pet.feed();
        assert!(pet.hunger <= 100);
    }

    #[test]
    fn test_feed_increases_happiness() {
        let mut pet = PetState::new();
        pet.happiness = 50;
        pet.last_update = Utc::now().timestamp();
        pet.feed();
        assert!(pet.happiness > 50);
    }

    #[test]
    fn test_feed_sets_activity_eating() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.feed();
        assert_eq!(pet.activity, Activity::Eating);
        assert!(pet.activity_until.is_some());
        assert!(pet.last_feed_time.is_some());
    }

    #[test]
    fn test_feed_rejected_while_sleeping() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(Utc::now().timestamp() + 7200);
        pet.hunger = 50;
        pet.feed();
        assert_eq!(pet.hunger, 50); // unchanged
        assert_eq!(pet.activity, Activity::Sleeping); // still sleeping
    }

    #[test]
    fn test_play_increases_happiness() {
        let mut pet = PetState::new();
        pet.happiness = 50;
        pet.last_update = Utc::now().timestamp();
        pet.play();
        assert!(pet.happiness > 50);
    }

    #[test]
    fn test_play_drains_energy() {
        let mut pet = PetState::new();
        pet.energy = 50;
        pet.last_update = Utc::now().timestamp();
        pet.play();
        assert!(pet.energy < 50);
    }

    #[test]
    fn test_play_drains_hunger() {
        let mut pet = PetState::new();
        pet.hunger = 50;
        pet.last_update = Utc::now().timestamp();
        pet.play();
        assert!(pet.hunger < 50);
    }

    #[test]
    fn test_play_sets_activity_playing() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.play();
        assert_eq!(pet.activity, Activity::Playing);
        assert!(pet.activity_until.is_some());
        assert!(pet.last_play_time.is_some());
    }

    #[test]
    fn test_play_rejected_while_sleeping() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(Utc::now().timestamp() + 7200);
        pet.happiness = 50;
        pet.play();
        assert_eq!(pet.happiness, 50); // unchanged
    }

    #[test]
    fn test_sleep_sets_activity_sleeping() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.sleep();
        assert_eq!(pet.activity, Activity::Sleeping);
        assert!(pet.activity_until.is_some());
        // Sleep duration should be 1-4 hours
        let now = Utc::now().timestamp();
        let until = pet.activity_until.unwrap();
        let duration = until - now;
        assert!(duration >= 3600 && duration <= 14400);
    }

    #[test]
    fn test_sleep_noop_if_already_sleeping() {
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.activity = Activity::Sleeping;
        let original_until = Utc::now().timestamp() + 7200;
        pet.activity_until = Some(original_until);
        pet.sleep();
        // Should not change the existing sleep timer
        assert_eq!(pet.activity, Activity::Sleeping);
        assert_eq!(pet.activity_until, Some(original_until));
    }

    #[test]
    fn test_update_wakes_pet_when_sleep_expires() {
        let mut pet = PetState::new();
        let now = Utc::now().timestamp();
        pet.last_update = now;
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(now - 1); // already expired
        pet.update();
        assert_eq!(pet.activity, Activity::Idle);
        assert!(pet.activity_until.is_none());
    }

    #[test]
    fn test_no_action_when_dead() {
        let mut pet = PetState::new();
        pet.is_alive = false;
        pet.hunger = 10;
        pet.happiness = 10;
        pet.energy = 10;

        pet.feed();
        assert_eq!(pet.hunger, 10);

        pet.play();
        assert_eq!(pet.happiness, 10);

        pet.sleep();
        assert_eq!(pet.energy, 10);
    }

    #[test]
    fn test_energy_no_underflow() {
        let mut pet = PetState::new();
        pet.energy = 3;
        pet.last_update = Utc::now().timestamp();
        pet.play(); // -10 energy
        assert_eq!(pet.energy, 0); // saturating_sub prevents underflow
    }

    #[test]
    fn test_hunger_no_underflow_on_play() {
        let mut pet = PetState::new();
        pet.hunger = 2;
        pet.last_update = Utc::now().timestamp();
        pet.play(); // -5 hunger
        assert_eq!(pet.hunger, 0);
    }

    #[test]
    fn test_eat_snack_applies_effects_without_feed_cooldown() {
        let snack = super::super::economy::find_snack("cookie").unwrap();
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.hunger = 40;
        pet.happiness = 40;
        assert!(pet.eat_snack(snack).is_ok());
        assert_eq!(pet.hunger, 55);
        assert_eq!(pet.happiness, 50);
        assert_eq!(pet.activity, Activity::Eating);
        assert!(pet.last_feed_time.is_none()); // FEED cooldown untouched
    }

    #[test]
    fn test_eat_snack_caps_at_100_and_rejects_useless() {
        let cake = super::super::economy::find_snack("cake").unwrap();
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.hunger = 95;
        pet.happiness = 90;
        assert!(pet.eat_snack(cake).is_ok());
        assert_eq!((pet.hunger, pet.happiness), (100, 100));

        pet.activity = Activity::Idle;
        pet.activity_until = None;
        assert!(pet.eat_snack(cake).is_err()); // already full: nothing changes
    }

    #[test]
    fn test_eat_snack_rejected_when_sleeping_or_dead() {
        let candy = super::super::economy::find_snack("candy").unwrap();
        let mut pet = PetState::new();
        pet.last_update = Utc::now().timestamp();
        pet.hunger = 10;
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(Utc::now().timestamp() + 7200);
        assert!(pet.eat_snack(candy).is_err());
        assert_eq!(pet.hunger, 10);

        pet.activity = Activity::Idle;
        pet.is_alive = false;
        assert!(pet.eat_snack(candy).is_err());
    }

    #[test]
    fn test_status_content() {
        let mut pet = PetState::new();
        assert_eq!(pet.status(), "content");

        pet.hunger = 10;
        assert_eq!(pet.status(), "hungry");

        pet.hunger = 100;
        pet.energy = 10;
        assert_eq!(pet.status(), "tired");

        pet.energy = 100;
        pet.happiness = 10;
        assert_eq!(pet.status(), "sad");
    }

    #[test]
    fn test_status_sleeping() {
        let mut pet = PetState::new();
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(Utc::now().timestamp() + 7200);
        assert!(pet.status().starts_with("sleeping"));
    }

    #[test]
    fn test_status_eating() {
        let mut pet = PetState::new();
        pet.activity = Activity::Eating;
        assert_eq!(pet.status(), "eating");
    }

    #[test]
    fn test_status_playing() {
        let mut pet = PetState::new();
        pet.activity = Activity::Playing;
        assert_eq!(pet.status(), "playing");
    }

    // Absence as deep sleep (design/gdd/companion.md F5 and the P6 verification table)
    const T0: i64 = 1_700_000_000;
    const HOUR: i64 = 3600;
    const DAY: i64 = 24 * HOUR;
    const ABSENCES: [i64; 3] = [63 * HOUR, 9 * DAY, 30 * DAY];

    fn pet_left_at(hunger: u8, happiness: u8, energy: u8, health: u8) -> PetState {
        let mut pet = PetState::new();
        pet.hunger = hunger;
        pet.happiness = happiness;
        pet.energy = energy;
        pet.health = health;
        pet.last_update = T0;
        pet
    }

    fn stats(pet: &PetState) -> [u8; 4] {
        [pet.hunger, pet.happiness, pet.energy, pet.health]
    }

    /// The design table is computed in f64; the integer present rules may differ by up to 1
    fn assert_stats_near(pet: &PetState, expected: [f64; 4]) {
        let actual = stats(pet).map(f64::from);
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() <= 1.0, "expected {:?}, got {:?}", expected, actual);
        }
    }

    #[test]
    fn test_absence_from_full_stats_survives() {
        for gap in ABSENCES {
            let mut pet = pet_left_at(100, 100, 100, 100);
            pet.update_at(T0 + gap, 0);
            assert!(pet.is_alive, "died after {}s", gap);
            assert_stats_near(&pet, [30.0, 95.5, 100.0, 100.0]);
        }
    }

    #[test]
    fn test_absence_from_typical_friday_stats_survives() {
        for gap in ABSENCES {
            let mut pet = pet_left_at(70, 70, 50, 100);
            pet.update_at(T0 + gap, 0);
            assert!(pet.is_alive, "died after {}s", gap);
            assert_stats_near(&pet, [30.0, 65.5, 100.0, 100.0]);
        }
    }

    #[test]
    fn test_short_gap_uses_present_rules_only() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 600, 0);
        assert_eq!(stats(&pet), [98, 99, 99, 100]);
    }

    #[test]
    fn test_clock_moved_backwards_changes_nothing() {
        let mut pet = pet_left_at(70, 70, 50, 100);
        pet.age = 1234;
        pet.update_at(T0 - HOUR, 0);
        assert_eq!(stats(&pet), [70, 70, 50, 100]);
        assert_eq!(pet.age, 1234);
        assert_eq!(pet.last_update, T0 - HOUR);
        assert!(pet.is_alive);
    }

    #[test]
    fn test_needs_gap_clamped_to_offline_cap() {
        let mut at_cap = pet_left_at(70, 70, 50, 40);
        at_cap.update_at(T0 + 96 * HOUR, 0);
        let mut beyond_cap = pet_left_at(70, 70, 50, 40);
        beyond_cap.update_at(T0 + 200 * HOUR, 0);
        assert_eq!(stats(&beyond_cap), stats(&at_cap));
        // age is not limited by the needs cap
        assert_eq!(beyond_cap.age, (200 * HOUR) as u64);
    }

    #[test]
    fn test_age_grows_during_absence_up_to_cap() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 9 * DAY, 0);
        assert_eq!(pet.age, (9 * DAY) as u64);

        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 30 * DAY, 0);
        assert_eq!(pet.age, (14 * DAY) as u64);
    }

    #[test]
    fn test_near_death_pet_survives_if_first_30_minutes_do_not_kill() {
        let mut pet = pet_left_at(5, 70, 50, 3);
        pet.update_at(T0 + 63 * HOUR, 0);
        assert!(pet.is_alive);
        // First 30 minutes: hunger 5 - 6 -> 0, health 3 - 0.05 * 30 = 1.5, shown rounded as 2. Then nothing drops
        assert_eq!(pet.hunger, 0);
        assert_eq!(pet.health, 2);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn test_near_death_pet_dies_within_first_30_minutes() {
        let mut pet = pet_left_at(5, 70, 50, 1);
        pet.update_at(T0 + 63 * HOUR, 0);
        assert!(!pet.is_alive);
    }

    #[test]
    fn test_absence_keeps_idle_so_feed_is_accepted() {
        let mut pet = pet_left_at(70, 70, 50, 100);
        pet.update_at(T0 + 63 * HOUR, 0);
        assert_eq!(pet.activity, Activity::Idle);
        assert!(pet.activity_until.is_none());
    }

    #[test]
    fn test_sleep_that_ended_during_absence_wakes_up() {
        let mut pet = pet_left_at(70, 70, 50, 100);
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(T0 + 2 * HOUR);
        pet.update_at(T0 + 63 * HOUR, 0);
        assert_eq!(pet.activity, Activity::Idle);
        assert!(pet.activity_until.is_none());
        assert!(pet.is_alive);
    }

    // Away from the keyboard while the app keeps running (F5, P6-b)
    const MINUTE: i64 = 60;
    const POLL: i64 = 5;

    /// Updates every `step` seconds from `last_update` up to `end`, like the frontend polling.
    /// `idle_at(t)` is the input idle time the OS would report at `t`.
    fn poll_until(pet: &mut PetState, end: i64, step: i64, idle_at: impl Fn(i64) -> u64) {
        let mut now = pet.last_update;
        while now < end && pet.is_alive {
            now = (now + step).min(end);
            pet.update_at(now, idle_at(now));
        }
    }

    /// Idle time for a user whose last input was at `last_input`.
    fn idle_since(last_input: i64) -> impl Fn(i64) -> u64 {
        move |now| (now - last_input).max(0) as u64
    }

    #[test]
    fn test_idle_over_threshold_uses_deep_sleep_rules() {
        let mut pet = pet_left_at(100, 80, 60, 100);
        // The user left 40 minutes before T0. Multiplier 1.0, so hunger reaches the floor within the day
        poll_until_with_mult(&mut pet, T0 + 24 * HOUR, POLL, 1.0, idle_since(T0 - 40 * MINUTE));
        assert!(pet.is_alive);
        assert_eq!(pet.happiness, 80); // no decay while away
        assert_eq!(pet.hunger, 30); // stops at AWAY_HUNGER_FLOOR
        assert_eq!(pet.energy, 100);
        assert_eq!(pet.activity, Activity::Idle);
    }

    #[test]
    fn test_idle_under_threshold_uses_present_rules() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        // Idle 10 to 30 minutes: still present
        poll_until(&mut pet, T0 + 20 * MINUTE, POLL, idle_since(T0 - 10 * MINUTE));
        // 20 minutes of present decay: 0.2 * 20, 0.15 * 20, 0.1 * 20
        assert_eq!(stats(&pet), [96, 97, 98, 100]);
    }

    #[test]
    fn test_63h_away_with_app_running_survives() {
        let mut pet = pet_left_at(70, 70, 50, 100);
        // Friday 18:00 the user walks away from a Mac that stays on, polling every 5 seconds
        poll_until(&mut pet, T0 + 63 * HOUR, POLL, idle_since(T0));
        assert!(pet.is_alive);
        // Same as closing the app: the first 30 minutes use the present rules
        assert_stats_near(&pet, [30.0, 65.5, 100.0, 100.0]);
    }

    #[test]
    fn test_unreadable_idle_counts_as_present() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        poll_until(&mut pet, T0 + 20 * MINUTE, POLL, |_| 0);
        assert_eq!(stats(&pet), [96, 97, 98, 100]);
    }

    #[test]
    fn test_gap_and_idle_take_the_larger_away_part() {
        // A 2 hour gap where the user had been idle for 3 hours: idle covers the whole gap
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 2 * HOUR, (3 * HOUR) as u64);
        assert_eq!(pet.happiness, 100);
        // A 2 hour gap with fresh input: the gap rule still keeps only 30 minutes present
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 2 * HOUR, 0);
        assert_eq!(pet.happiness, 96); // 100 - 0.15 * 30 = 95.5
    }

    // Fractions are carried between updates (P2, F0)
    fn full_stats(pet: &PetState) -> [f64; 4] {
        [
            f64::from(pet.hunger) + pet.hunger_frac,
            f64::from(pet.happiness) + pet.happiness_frac,
            f64::from(pet.energy) + pet.energy_frac,
            f64::from(pet.health) + pet.health_frac,
        ]
    }

    #[test]
    fn test_twelve_5s_updates_equal_one_60s_update() {
        let mut polled = pet_left_at(50, 50, 50, 50);
        poll_until(&mut polled, T0 + MINUTE, POLL, |_| 0);
        let mut once = pet_left_at(50, 50, 50, 50);
        once.update_at(T0 + MINUTE, 0);

        assert_eq!(stats(&polled), stats(&once));
        for (p, o) in full_stats(&polled).iter().zip(full_stats(&once)) {
            assert!((p - o).abs() < 1e-9, "{:?} vs {:?}", full_stats(&polled), full_stats(&once));
        }
        // And the minute actually counted: hunger 50 - 0.2
        assert!((full_stats(&polled)[0] - 49.8).abs() < 1e-9);
    }

    #[test]
    fn test_one_hour_of_5s_updates_matches_rate_times_minutes() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        poll_until(&mut pet, T0 + HOUR, POLL, |_| 0);
        // 0.2 * 60 = 12, 0.15 * 60 = 9, 0.1 * 60 = 6. The truncating version lost all of it
        assert_eq!(stats(&pet), [88, 91, 94, 100]);
    }

    #[test]
    fn test_save_without_fraction_fields_still_loads() {
        let mut json = serde_json::to_value(pet_left_at(70, 60, 50, 40)).unwrap();
        let fields = json.as_object_mut().unwrap();
        for key in ["hunger_frac", "happiness_frac", "energy_frac", "health_frac"] {
            assert!(fields.remove(key).is_some(), "{} should be saved", key);
        }
        let pet: PetState = serde_json::from_value(json).unwrap();
        assert_eq!(stats(&pet), [70, 60, 50, 40]);
        assert_eq!(full_stats(&pet), [70.0, 60.0, 50.0, 40.0]);
    }

    #[test]
    fn test_public_stats_stay_whole_numbers_in_json() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 7, 0);
        let json = serde_json::to_value(&pet).unwrap();
        for key in ["hunger", "happiness", "energy", "health"] {
            assert!(json[key].is_u64(), "{} should be an integer, got {}", key, json[key]);
        }
    }

    // Step by step integration (F0-3)
    #[test]
    fn test_sleep_end_splits_the_update() {
        let mut pet = pet_left_at(80, 80, 50, 100);
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(T0 + 10 * MINUTE);
        pet.update_at(T0 + 30 * MINUTE, 0);
        // 10 sleeping minutes: hunger -1, energy +5. Then 20 awake minutes: hunger -4, happiness -3, energy -2
        assert_eq!(stats(&pet), [75, 77, 53, 100]);
        assert_eq!(pet.activity, Activity::Idle);
    }

    #[test]
    fn test_health_recovery_is_judged_every_minute() {
        let mut pet = pet_left_at(52, 80, 80, 50);
        pet.update_at(T0 + 30 * MINUTE, 0);
        // hunger stays above 50 for the first 10 minutes only: +0.3 * 10
        assert_eq!(pet.hunger, 46);
        assert_eq!(pet.health, 53);
    }

    /// `poll_until` with the stage multiplier pinned, for the multiplier-1.0 rows of the design tables.
    /// A real pet turns Adult after 6 hours of age, so it would leave 1.0 partway through.
    fn poll_until_with_mult(pet: &mut PetState, end: i64, step: i64, mult: f64, idle_at: impl Fn(i64) -> u64) {
        let mut now = pet.last_update;
        while now < end && pet.is_alive {
            now = (now + step).min(end);
            pet.update_at_with_mult(now, idle_at(now), mult);
        }
    }

    // F4: time to death while present the whole time, multiplier 1.0 (Egg to Child)
    #[test]
    fn test_f4_continuous_presence_dies_at_t_allzero() {
        let mut pet = pet_left_at(100, 100, 100, 100);
        poll_until_with_mult(&mut pet, T0 + 30 * HOUR, MINUTE, 1.0, |_| 0);
        assert!(!pet.is_alive);
        let minutes = (pet.last_update - T0) / MINUTE;
        // T_allzero = max(100/0.2, 100/0.15, 100/0.1) = 1000 min, before T_health = 90/0.2 + 100/0.05 = 2450 min
        assert!((990..=1000).contains(&minutes), "died after {} minutes", minutes);
        assert!(pet.health > 0, "should die of neglect, not health");
    }

    /// Daily care on weekdays, as times after 9:00 (the design F4 care table: FEED 9:30, PLAY 14:00).
    struct Care {
        feeds: &'static [i64],
        plays: &'static [i64],
    }

    const NO_CARE: Care = Care { feeds: &[], plays: &[] };
    const FEED_AND_PLAY: Care = Care { feeds: &[30 * MINUTE], plays: &[5 * HOUR] };

    /// Present `hours_per_day` from 9:00 on weekdays, away the rest of the time, the app always running.
    /// `mult` pins the stage multiplier; `None` lets the pet's stage decide it.
    /// Returns (business day of death, scheduled present hours, present hours including the 30 minute borders),
    /// or `None` if the pet is still alive after `days`.
    fn simulate_workdays(
        mut pet: PetState,
        mult: Option<f64>,
        hours_per_day: i64,
        care: &Care,
        days: i64,
    ) -> Option<(i64, f64, f64)> {
        let monday_9am = pet.last_update;
        let mut last_input = monday_9am;
        let mut scheduled = 0;
        let mut borders = 0;
        let mut now = monday_9am;
        while pet.is_alive && now < monday_9am + days * DAY {
            let since_monday = now - monday_9am;
            let (day, time_of_day) = (since_monday / DAY, since_monday % DAY);
            let weekday = day % 7 < 5;
            let at_desk = weekday && time_of_day < hours_per_day * HOUR;
            if at_desk {
                last_input = now;
                if care.feeds.contains(&time_of_day) {
                    pet.apply_feed(now);
                }
                if care.plays.contains(&time_of_day) {
                    pet.apply_play(now);
                }
            }
            now += MINUTE;
            let idle = (now - last_input).max(0) as u64;
            let step_mult = mult.unwrap_or_else(|| super::super::needs::stage_decay_mult(pet.stage));
            pet.update_at_with_mult(now, idle, step_mult);
            if at_desk {
                scheduled += MINUTE;
            } else if idle <= super::super::needs::AWAY_THRESHOLD_SECS {
                borders += MINUTE;
            }
        }
        if pet.is_alive {
            return None;
        }
        let day = (pet.last_update - 1 - monday_9am) / DAY;
        let business_day = day / 7 * 5 + day % 7 + 1;
        let hours = |secs: i64| secs as f64 / HOUR as f64;
        Some((business_day, hours(scheduled), hours(scheduled + borders)))
    }

    fn f4_schedule_death(hours_per_day: i64) -> (i64, f64, f64) {
        simulate_workdays(pet_left_at(100, 100, 100, 100), Some(1.0), hours_per_day, &NO_CARE, 60)
            .expect("never died")
    }

    #[test]
    fn test_f4_table_nine_hours_a_day() {
        // Design F4: multiplier 1.0, 9 h/day -> 5th business day (38.8 / 40.8 present hours)
        let (business_day, scheduled, with_borders) = f4_schedule_death(9);
        assert_eq!(business_day, 5);
        assert!((scheduled - 38.8).abs() <= 0.25, "scheduled {}", scheduled);
        assert!((with_borders - 40.8).abs() <= 0.25, "with borders {}", with_borders);
    }

    #[test]
    fn test_f4_table_four_hours_a_day() {
        // Design F4: multiplier 1.0, 4 h/day -> 9th business day (35.5 / 39.5 present hours)
        let (business_day, scheduled, with_borders) = f4_schedule_death(4);
        assert_eq!(business_day, 9);
        assert!((scheduled - 35.5).abs() <= 0.25, "scheduled {}", scheduled);
        assert!((with_borders - 39.5).abs() <= 0.25, "with borders {}", with_borders);
    }

    // Adult decay multiplier 0.2 (design R5, F4, F5; decision 18)
    fn adult_left_at(hunger: u8, happiness: u8, energy: u8, health: u8) -> PetState {
        let mut pet = pet_left_at(hunger, happiness, energy, health);
        pet.stage = EvolutionStage::Adult;
        pet.age = 30 * DAY as u64;
        pet
    }

    #[test]
    fn test_adult_decays_at_a_fifth_of_the_rate() {
        let mut pet = adult_left_at(100, 100, 100, 100);
        poll_until(&mut pet, T0 + 600 * MINUTE, POLL, |_| 0);
        // 600 minutes: hunger 0.04, happiness 0.03, energy 0.02 per minute
        assert_eq!(stats(&pet), [76, 82, 88, 100]);
    }

    #[test]
    fn test_adult_continuous_presence_dies_around_71_hours() {
        let mut pet = adult_left_at(100, 100, 100, 100);
        poll_until(&mut pet, T0 + 69 * HOUR, POLL, |_| 0);
        assert!(pet.is_alive);
        assert_eq!((pet.hunger, pet.happiness), (0, 0));
        assert!(pet.energy.abs_diff(17) <= 1, "energy {}", pet.energy);
        assert!(pet.health.abs_diff(5) <= 1, "health {}", pet.health);

        poll_until(&mut pet, T0 + 72 * HOUR, POLL, |_| 0);
        assert!(!pet.is_alive);
        // T_death = T_health = 90/0.04 + 100/0.05 = 4250 minutes = 70.8 hours (T_allzero is 83.3 hours)
        let hours = (pet.last_update - T0) as f64 / HOUR as f64;
        assert!((hours - 70.8).abs() <= 1.0, "died after {} hours", hours);
        assert!(pet.energy > 0, "should die of health, not neglect");
    }

    #[test]
    fn test_adult_nine_hours_a_day_without_care_dies_on_7th_business_day() {
        // Design F4: Adult (0.2), 9 h/day -> 7th business day (57.7 / 60.7 present hours). AC range 56-59 / 59-62
        let (business_day, scheduled, with_borders) =
            simulate_workdays(adult_left_at(100, 100, 100, 100), None, 9, &NO_CARE, 60).expect("never died");
        assert_eq!(business_day, 7);
        assert!((56.0..=59.0).contains(&scheduled), "scheduled {}", scheduled);
        assert!((59.0..=62.0).contains(&with_borders), "with borders {}", with_borders);
    }

    #[test]
    fn test_adult_feed_and_play_once_a_day_survives_130_days() {
        // Design F4 care table: 0.2 with FEED 1 + PLAY 1 survives (decision 18)
        let death = simulate_workdays(adult_left_at(100, 100, 100, 100), None, 9, &FEED_AND_PLAY, 130);
        assert_eq!(death, None);
    }

    #[test]
    fn test_old_multiplier_dies_despite_feed_and_play() {
        // Why 0.4 was dropped: FEED 1 + PLAY 1 still dies on the 9th business day (design F4 care table)
        let death = simulate_workdays(adult_left_at(100, 100, 100, 100), Some(0.4), 9, &FEED_AND_PLAY, 130);
        assert_eq!(death.map(|(business_day, _, _)| business_day), Some(9));
    }

    #[test]
    fn test_adult_absence_from_full_stats_survives() {
        for gap in ABSENCES {
            let mut pet = adult_left_at(100, 100, 100, 100);
            pet.update_at(T0 + gap, 0);
            assert!(pet.is_alive, "died after {}s", gap);
            // Design verification table, Adult 0.2 row
            assert_stats_near(&pet, [30.0, 99.1, 100.0, 100.0]);
        }
    }

    #[test]
    fn test_adult_absence_from_typical_friday_stats_survives() {
        for gap in ABSENCES {
            let mut pet = adult_left_at(70, 70, 50, 100);
            pet.update_at(T0 + gap, 0);
            assert!(pet.is_alive, "died after {}s", gap);
            assert_stats_near(&pet, [30.0, 69.1, 100.0, 100.0]);
        }
    }

    #[test]
    fn test_adult_absence_hunger_converges_slower() {
        // Adult away hunger: 0.2 * 0.5 * 0.2 = 0.02 per minute. 24 hours: 98.8 - 0.02 * 1410 = 70.6
        let mut pet = adult_left_at(100, 100, 100, 100);
        pet.update_at(T0 + 24 * HOUR, 0);
        assert_stats_near(&pet, [70.6, 99.1, 100.0, 100.0]);
    }

    #[test]
    fn test_evolving_update_uses_the_stage_at_its_start() {
        // A Child one minute before adulthood, then a 63 hour gap: the whole gap keeps the Child rate
        let mut pet = pet_left_at(100, 100, 100, 100);
        pet.stage = EvolutionStage::Child;
        pet.age = 6 * HOUR as u64 - 60;
        pet.update_at(T0 + 63 * HOUR, 0);
        assert_eq!(pet.stage, EvolutionStage::Adult);
        assert_stats_near(&pet, [30.0, 95.5, 100.0, 100.0]);
    }

    #[test]
    fn test_young_stages_keep_full_rate() {
        for stage in [EvolutionStage::Egg, EvolutionStage::Baby, EvolutionStage::Child] {
            let mut pet = pet_left_at(100, 100, 100, 100);
            pet.stage = stage;
            pet.age = 0;
            poll_until(&mut pet, T0 + HOUR, POLL, |_| 0);
            // Under an hour of age, so none of them reaches Adult: 0.2 * 60, 0.15 * 60, 0.1 * 60
            assert_eq!(stats(&pet), [88, 91, 94, 100], "{:?}", stage);
        }
    }
}

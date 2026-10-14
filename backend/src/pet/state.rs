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
        }
    }

    pub fn update(&mut self) {
        if !self.is_alive {
            return;
        }

        let now = Utc::now().timestamp();
        let elapsed = (now - self.last_update) as u64;

        // Update age
        self.age += elapsed;

        // Check if current activity has ended
        if let Some(until) = self.activity_until {
            if now >= until {
                self.activity = Activity::Idle;
                self.activity_until = None;
            }
        }

        // Update needs based on elapsed time
        super::needs::apply_decay(self, elapsed);

        // Check evolution
        super::evolution::check_evolution(self);

        // Check death conditions
        super::lifecycle::check_death(self);

        self.last_update = now;
    }

    pub fn feed(&mut self) {
        self.update();
        if !self.is_alive {
            return;
        }

        // Can't feed while sleeping
        if self.activity == Activity::Sleeping {
            return;
        }

        let now = Utc::now().timestamp();
        self.hunger = (self.hunger + 30).min(100);
        self.happiness = (self.happiness + 5).min(100);
        self.last_feed_time = Some(now);
        self.activity = Activity::Eating;
        self.activity_until = Some(now + EATING_DURATION_SECS);
    }

    pub fn play(&mut self) {
        self.update();
        if !self.is_alive {
            return;
        }

        // Can't play while sleeping
        if self.activity == Activity::Sleeping {
            return;
        }

        let now = Utc::now().timestamp();
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
}

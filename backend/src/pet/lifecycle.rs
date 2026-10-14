use super::state::{PetState, Activity};

pub fn check_death(pet: &mut PetState) {
    if !pet.is_alive {
        return;
    }

    // Death conditions (only from complete neglect)
    if pet.health == 0 {
        pet.is_alive = false;
        println!("Pet died from poor health!");
    }

    // Only die from extreme neglect (all stats must be critical)
    if pet.hunger == 0 && pet.happiness == 0 && pet.energy == 0 {
        pet.is_alive = false;
        println!("Pet died from neglect!");
    }
}

pub fn revive(pet: &mut PetState) {
    // Reset to new egg
    let now = chrono::Utc::now().timestamp();
    pet.stage = super::evolution::EvolutionStage::Egg;
    pet.age = 0;
    pet.hunger = 100;
    pet.happiness = 100;
    pet.energy = 100;
    pet.health = 100;
    pet.hunger_frac = 0.0;
    pet.happiness_frac = 0.0;
    pet.energy_frac = 0.0;
    pet.health_frac = 0.0;
    pet.is_alive = true;
    pet.birth_time = now;
    pet.last_update = now;
    pet.activity = Activity::Idle;
    pet.activity_until = None;
    pet.last_feed_time = None;
    pet.last_play_time = None;
    println!("Pet revived as a new egg!");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::evolution::EvolutionStage;

    #[test]
    fn test_death_from_zero_health() {
        let mut pet = PetState::new();
        pet.health = 0;
        check_death(&mut pet);
        assert!(!pet.is_alive);
    }

    #[test]
    fn test_death_from_total_neglect() {
        let mut pet = PetState::new();
        pet.hunger = 0;
        pet.happiness = 0;
        pet.energy = 0;
        pet.health = 50; // health not zero but all stats are
        check_death(&mut pet);
        assert!(!pet.is_alive);
    }

    #[test]
    fn test_alive_when_one_stat_nonzero() {
        let mut pet = PetState::new();
        pet.hunger = 0;
        pet.happiness = 0;
        pet.energy = 1; // one stat above zero
        pet.health = 1;
        check_death(&mut pet);
        assert!(pet.is_alive);
    }

    #[test]
    fn test_alive_with_low_stats() {
        let mut pet = PetState::new();
        pet.hunger = 1;
        pet.happiness = 1;
        pet.energy = 1;
        pet.health = 1;
        check_death(&mut pet);
        assert!(pet.is_alive);
    }

    #[test]
    fn test_no_double_death() {
        let mut pet = PetState::new();
        pet.is_alive = false;
        pet.health = 0;
        check_death(&mut pet); // should not panic or change state
        assert!(!pet.is_alive);
    }

    #[test]
    fn test_revive_resets_to_egg() {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Adult;
        pet.is_alive = false;
        pet.health = 0;
        pet.hunger = 0;
        pet.age = 99999;
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(99999);
        pet.last_feed_time = Some(99999);
        pet.last_play_time = Some(99999);

        revive(&mut pet);

        assert!(pet.is_alive);
        assert_eq!(pet.stage, EvolutionStage::Egg);
        assert_eq!(pet.age, 0);
        assert_eq!(pet.hunger, 100);
        assert_eq!(pet.happiness, 100);
        assert_eq!(pet.energy, 100);
        assert_eq!(pet.health, 100);
        assert_eq!(pet.activity, Activity::Idle);
        assert!(pet.activity_until.is_none());
        assert!(pet.last_feed_time.is_none());
        assert!(pet.last_play_time.is_none());
    }

    #[test]
    fn test_revive_clears_carried_fractions() {
        let mut pet = PetState::new();
        pet.is_alive = false;
        pet.hunger_frac = -0.4;
        pet.happiness_frac = 0.3;
        pet.energy_frac = -0.2;
        pet.health_frac = 0.1;

        revive(&mut pet);

        assert_eq!(
            [pet.hunger_frac, pet.happiness_frac, pet.energy_frac, pet.health_frac],
            [0.0; 4]
        );
    }
}

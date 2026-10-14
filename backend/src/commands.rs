use tauri::{AppHandle, State};
use serde::Serialize;
use std::time::Instant;
use chrono::{Local, NaiveDate, Utc};
use crate::{AppState, game_session, pet::{PetState, GameStats, Achievement, achievements, economy}};
use crate::companion::{CareKind, CompanionState};
use crate::pet::state::{Activity, FEED_COOLDOWN_SECS, PLAY_COOLDOWN_SECS};
use crate::storage;

fn save_pet(pet: &PetState) {
    if let Err(e) = storage::save_state(pet) {
        eprintln!("Failed to save pet state: {}", e);
    }
}

fn save_stats(stats: &GameStats) {
    if let Err(e) = storage::save_stats(stats) {
        eprintln!("Failed to save game stats: {}", e);
    }
}

fn save_companion(companion: &CompanionState) {
    if let Err(e) = storage::save_companion(companion) {
        eprintln!("Failed to save companion: {}", e);
    }
}

/// Refuses an action the pet cannot take right now, with the message shown to the user.
fn check_care(pet: &PetState, kind: CareKind, now: i64) -> Result<(), String> {
    if pet.activity == Activity::Sleeping {
        return Err(match kind {
            CareKind::Sleep => "dot is already sleeping",
            CareKind::Feed | CareKind::Play => "dot is sleeping",
        }
        .to_string());
    }
    let (last, cooldown, name) = match kind {
        CareKind::Feed => (pet.last_feed_time, FEED_COOLDOWN_SECS, "feed"),
        CareKind::Play => (pet.last_play_time, PLAY_COOLDOWN_SECS, "play"),
        CareKind::Sleep => return Ok(()),
    };
    if let Some(last) = last {
        let elapsed = now - last;
        if elapsed < cooldown {
            let remaining = cooldown - elapsed;
            let mins = remaining / 60;
            return Err(format!("{} cooldown: {}m left", name, mins + 1));
        }
    }
    Ok(())
}

/// FEED / PLAY / SLEEP. Counts towards Bond only when it went through.
fn care(
    pet: &mut PetState,
    companion: &mut CompanionState,
    kind: CareKind,
    now: i64,
    today: NaiveDate,
) -> Result<(), String> {
    check_care(pet, kind, now)?;
    // Bring the needs up to now first, so the need is read as it is just before the action
    pet.update();
    let need_before = kind.need(pet);
    let (last_feed, last_play) = (pet.last_feed_time, pet.last_play_time);
    // A pet that died during the update ignores the action, so check that it took effect
    let went_through = match kind {
        CareKind::Feed => {
            pet.feed();
            pet.last_feed_time != last_feed
        }
        CareKind::Play => {
            pet.play();
            pet.last_play_time != last_play
        }
        CareKind::Sleep => {
            pet.sleep();
            pet.activity == Activity::Sleeping
        }
    };
    if went_through {
        companion.record_care(pet, need_before, today);
    }
    Ok(())
}

/// Shared body of the FEED / PLAY / SLEEP commands.
fn care_command(
    state: &AppState,
    kind: CareKind,
    record: fn(&mut GameStats),
) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    {
        let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
        care(&mut pet, &mut companion, kind, Utc::now().timestamp(), Local::now().date_naive())?;
        save_companion(&companion);
    }
    save_pet(&pet);

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    record(&mut stats);
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[derive(Serialize, Clone)]
pub struct PetResponse {
    #[serde(flatten)]
    pub pet: PetState,
    pub status: String,
}

impl PetResponse {
    fn from(pet: &PetState) -> Self {
        Self {
            status: pet.status(),
            pet: pet.clone(),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ActionResult {
    #[serde(flatten)]
    pub pet: PetResponse,
    pub new_achievements: Vec<Achievement>,
}

impl ActionResult {
    fn from(pet: &PetState, new_achievements: Vec<Achievement>) -> Self {
        Self {
            pet: PetResponse::from(pet),
            new_achievements,
        }
    }
}

#[tauri::command]
pub fn get_pet_state(state: State<AppState>) -> Result<PetResponse, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    pet.update();
    save_pet(&pet);
    {
        let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
        companion.advance_day(&pet, Local::now().date_naive());
        save_companion(&companion);
    }

    // Check achievements on poll too (for evolution-based ones)
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let _ = achievements::check_achievements(&mut stats, &pet);
    stats.accrue_passive(Utc::now().timestamp(), pet.is_alive);
    save_stats(&stats);

    Ok(PetResponse::from(&pet))
}

#[tauri::command]
pub fn feed_pet(state: State<AppState>) -> Result<ActionResult, String> {
    care_command(&state, CareKind::Feed, GameStats::record_feed)
}

#[tauri::command]
pub fn play_with_pet(state: State<AppState>) -> Result<ActionResult, String> {
    care_command(&state, CareKind::Play, GameStats::record_play)
}

#[tauri::command]
pub fn put_to_sleep(state: State<AppState>) -> Result<ActionResult, String> {
    care_command(&state, CareKind::Sleep, GameStats::record_sleep)
}

#[tauri::command]
pub fn revive_pet(state: State<AppState>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    crate::pet::lifecycle::revive(&mut pet);
    save_pet(&pet);
    {
        let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
        companion.reset_for_new_pet();
        save_companion(&companion);
    }

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    stats.record_revive();
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn get_game_stats(state: State<AppState>) -> Result<GameStats, String> {
    let stats = state.stats.lock().map_err(|e| e.to_string())?;
    Ok(stats.clone())
}

#[tauri::command]
pub fn get_achievements(state: State<AppState>) -> Result<Vec<Achievement>, String> {
    let stats = state.stats.lock().map_err(|e| e.to_string())?;
    Ok(achievements::get_all_achievements(&stats))
}

/// What happened to a broken save at startup ("SAVE RECOVERED" / "SAVE BACKED UP").
/// Returned once, then cleared.
#[tauri::command]
pub fn get_load_warning(state: State<AppState>) -> Option<String> {
    state.take_load_warning()
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_snacks() -> Vec<economy::Snack> {
    economy::SNACKS.to_vec()
}

#[derive(Serialize, Clone)]
pub struct RewardResult {
    pub earned: u64,
    pub coins: u64,
    /// The daily cap for mini game coins cut this reward.
    pub capped: bool,
}

/// Called by the UI when a mini game starts. The token must be passed back
/// to `report_game_result` to earn coins.
#[tauri::command]
pub fn start_game(state: State<AppState>, game: String) -> Result<String, String> {
    let mut sessions = state.game_sessions.lock().map_err(|e| e.to_string())?;
    sessions.start(&game, Instant::now())
}

/// Called by the UI when a mini game ends. Games never cost coins.
#[tauri::command]
pub fn report_game_result(
    state: State<AppState>,
    token: String,
    game: String,
    outcome: String,
    moves: Option<u32>,
) -> Result<RewardResult, String> {
    let mut sessions = state.game_sessions.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let today = Local::now().format("%Y-%m-%d").to_string();
    let reward = game_session::claim_reward(
        &mut sessions, &mut stats, &token, &game, &outcome, moves, Instant::now(), &today,
    )?;
    save_stats(&stats);
    Ok(RewardResult { earned: reward.earned, coins: stats.coins, capped: reward.capped })
}

#[tauri::command]
pub fn buy_snack(state: State<AppState>, snack_id: String) -> Result<ActionResult, String> {
    let snack = economy::find_snack(&snack_id).ok_or_else(|| "unknown snack".to_string())?;

    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;

    if stats.coins < snack.price {
        return Err(format!("not enough coins ({} needed)", snack.price));
    }

    // Validate and apply to the pet first, so coins are only spent when the snack is eaten.
    pet.eat_snack(snack)?;
    stats.try_spend(snack.price);
    save_pet(&pet);
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::EvolutionStage;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
    }

    fn adult() -> (PetState, CompanionState) {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Adult;
        let mut companion = CompanionState::default();
        companion.advance_day(&pet, today());
        (pet, companion)
    }

    fn now() -> i64 {
        Utc::now().timestamp()
    }

    #[test]
    fn feed_with_hunger_below_80_counts() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 50;

        care(&mut pet, &mut companion, CareKind::Feed, now(), today()).unwrap();

        assert_eq!(pet.hunger, 80);
        assert_eq!(companion.today.n_care, 1);
    }

    #[test]
    fn play_and_sleep_count_by_their_own_need() {
        let (mut pet, mut companion) = adult();
        pet.happiness = 40;
        pet.energy = 40;

        care(&mut pet, &mut companion, CareKind::Play, now(), today()).unwrap();
        pet.activity = Activity::Idle;
        care(&mut pet, &mut companion, CareKind::Sleep, now(), today()).unwrap();

        assert_eq!(pet.activity, Activity::Sleeping);
        assert_eq!(companion.today.n_care, 2);
    }

    #[test]
    fn feed_with_hunger_at_80_or_more_does_not_count() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 80;
        // Only hunger decides; another low need does not matter
        pet.happiness = 10;

        care(&mut pet, &mut companion, CareKind::Feed, now(), today()).unwrap();

        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn refused_feed_during_cooldown_does_not_count() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 10;
        let t = now();
        pet.last_feed_time = Some(t - 60);

        let result = care(&mut pet, &mut companion, CareKind::Feed, t, today());

        assert_eq!(result, Err("feed cooldown: 30m left".to_string()));
        assert_eq!(pet.hunger, 10);
        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn refused_care_while_sleeping_does_not_count() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 10;
        pet.energy = 10;
        pet.activity = Activity::Sleeping;
        pet.activity_until = Some(now() + 3600);

        assert_eq!(
            care(&mut pet, &mut companion, CareKind::Feed, now(), today()),
            Err("dot is sleeping".to_string())
        );
        assert_eq!(
            care(&mut pet, &mut companion, CareKind::Sleep, now(), today()),
            Err("dot is already sleeping".to_string())
        );
        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn care_before_adult_does_not_count() {
        let mut pet = PetState::new();
        pet.stage = EvolutionStage::Child;
        pet.hunger = 10;
        let mut companion = CompanionState::default();

        care(&mut pet, &mut companion, CareKind::Feed, now(), today()).unwrap();

        assert_eq!(pet.hunger, 40);
        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn care_for_a_pet_that_dies_in_the_update_does_not_count() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 10;
        pet.happiness = 10;
        pet.activity = Activity::Eating;
        pet.activity_until = Some(now() + 30);
        // The update before the action finds health at 0 and the pet dies
        pet.health = 0;

        care(&mut pet, &mut companion, CareKind::Feed, now(), today()).unwrap();

        assert!(!pet.is_alive);
        assert_eq!(companion.today.n_care, 0);
    }

    #[test]
    fn care_for_a_dead_pet_does_not_count() {
        let (mut pet, mut companion) = adult();
        pet.hunger = 10;
        pet.is_alive = false;

        care(&mut pet, &mut companion, CareKind::Feed, now(), today()).unwrap();

        assert_eq!(companion.today.n_care, 0);
    }
}

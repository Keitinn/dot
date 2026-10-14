use tauri::{AppHandle, State};
use serde::Serialize;
use std::time::Instant;
use chrono::{Local, Utc};
use crate::{AppState, game_session, pet::{PetState, GameStats, Achievement, achievements, economy}};
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

    // Check achievements on poll too (for evolution-based ones)
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let _ = achievements::check_achievements(&mut stats, &pet);
    stats.accrue_passive(Utc::now().timestamp(), pet.is_alive);
    save_stats(&stats);

    Ok(PetResponse::from(&pet))
}

#[tauri::command]
pub fn feed_pet(state: State<AppState>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;

    // Check sleeping
    if pet.activity == Activity::Sleeping {
        return Err("dot is sleeping".to_string());
    }

    // Check cooldown
    let now = Utc::now().timestamp();
    if let Some(last) = pet.last_feed_time {
        let elapsed = now - last;
        if elapsed < FEED_COOLDOWN_SECS {
            let remaining = FEED_COOLDOWN_SECS - elapsed;
            let mins = remaining / 60;
            return Err(format!("feed cooldown: {}m left", mins + 1));
        }
    }

    pet.feed();
    save_pet(&pet);

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    stats.record_feed();
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn play_with_pet(state: State<AppState>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;

    // Check sleeping
    if pet.activity == Activity::Sleeping {
        return Err("dot is sleeping".to_string());
    }

    // Check cooldown
    let now = Utc::now().timestamp();
    if let Some(last) = pet.last_play_time {
        let elapsed = now - last;
        if elapsed < PLAY_COOLDOWN_SECS {
            let remaining = PLAY_COOLDOWN_SECS - elapsed;
            let mins = remaining / 60;
            return Err(format!("play cooldown: {}m left", mins + 1));
        }
    }

    pet.play();
    save_pet(&pet);

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    stats.record_play();
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn put_to_sleep(state: State<AppState>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;

    if pet.activity == Activity::Sleeping {
        return Err("dot is already sleeping".to_string());
    }

    pet.sleep();
    save_pet(&pet);

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    stats.record_sleep();
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    save_stats(&stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn revive_pet(state: State<AppState>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    crate::pet::lifecycle::revive(&mut pet);
    save_pet(&pet);

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

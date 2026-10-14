use tauri::{AppHandle, Manager, State};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::MutexGuard;
use std::time::Instant;
use chrono::{Local, NaiveDate, Timelike, Utc};
use crate::{AppState, game_session, pet::{PetState, GameStats, Achievement, achievements, economy}};
use rand::{rngs::StdRng, SeedableRng};
use crate::companion::{self, CareKind, CompanionState, OnboardingAnswer, OpenResult, SaidLine, SettingsUpdate, SettingsView};
use crate::pet::state::{Activity, FEED_COOLDOWN_SECS, PLAY_COOLDOWN_SECS};
use crate::storage;
use crate::sync::runtime::{FetchOutcome, READ_TIMEOUT};
use crate::sync::{SyncRuntime, SyncSettingsView, SyncStatusView};

fn save_pet(pet: &PetState) {
    if let Err(e) = storage::save_state(pet) {
        eprintln!("Failed to save pet state: {}", e);
    }
}

fn save_companion_in(dir: Option<&Path>, companion: &CompanionState) {
    if let Some(dir) = dir {
        log_save_error("companion", storage::save_companion_in(dir, companion));
    }
}

fn save_pet_in(dir: Option<&Path>, pet: &PetState) {
    if let Some(dir) = dir {
        log_save_error("pet state", storage::save_state_in(dir, pet));
    }
}

fn save_stats_in(dir: Option<&Path>, stats: &GameStats) {
    if let Some(dir) = dir {
        log_save_error("game stats", storage::save_stats_in(dir, stats));
    }
}

fn lock_sync(state: &AppState) -> Result<MutexGuard<'_, SyncRuntime>, String> {
    state.sync.lock().map_err(|e| e.to_string())
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

/// Shared body of the FEED / PLAY / SLEEP commands, saving into `dir`.
pub(crate) fn care_at(
    state: &AppState,
    kind: CareKind,
    record: fn(&mut GameStats),
    now: i64,
    today: NaiveDate,
    dir: Option<&Path>,
) -> Result<ActionResult, String> {
    // pet → companion → stats → sync, as AppState requires
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let companion = {
        let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
        care(&mut pet, &mut companion, kind, now, today)?;
        save_companion_in(dir, &companion);
        companion
    };
    save_pet_in(dir, &pet);

    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let earned_before = stats.total_coins_earned;
    record(&mut stats);
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    {
        let mut sync = lock_sync(state)?;
        if sync.is_on() {
            sync.record_care(kind, &stats, earned_before);
            sync.observe_pet(&pet);
            sync.observe_companion(&companion);
            sync.observe_stats(&stats);
            // Before the stats: a ledger that missed this care would take it back at the next merge
            sync.write();
        }
    }
    drop(companion);
    save_stats_in(dir, &stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

fn care_command(state: &AppState, kind: CareKind, record: fn(&mut GameStats)) -> Result<ActionResult, String> {
    let dir = save_dir();
    care_at(state, kind, record, Utc::now().timestamp(), Local::now().date_naive(), dir.as_deref())
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

/// How often the background tick brings the pet up to date (design SIGNAL_TICK).
/// Short enough that a tick gap never reaches AWAY_THRESHOLD while the app runs.
pub const SIGNAL_TICK_SECS: u64 = 60;

/// When a tick happens. Taken as an argument so tests can set the time and idle.
#[derive(Clone, Copy, Debug)]
pub struct Moment {
    /// Unix seconds.
    pub now: i64,
    /// Seconds since the last user input (F5).
    pub idle_secs: u64,
    /// The local date, for the companion's daily settlement.
    pub today: NaiveDate,
}

impl Moment {
    pub fn current() -> Self {
        Self {
            now: Utc::now().timestamp(),
            idle_secs: crate::signals::idle_seconds(),
            today: Local::now().date_naive(),
        }
    }
}

/// Brings everything that moves with time up to `at`: the pet, the companion's
/// day, achievements (evolution-based ones) and passive income. Called by the
/// background tick whether or not the popover is shown, and by the UI poll.
/// Saves into `dir`; with no directory, or when a save fails, it only logs.
/// With sync on, what it pays goes into the ledger; the folder is left alone.
pub fn tick(state: &AppState, at: Moment, dir: Option<&Path>) -> Result<PetResponse, String> {
    run_tick(state, at, dir, SyncPass::Record)
}

/// The background tick: `tick`, and with sync on also a look at the folder.
/// Device files that changed are read and merged into the game, and this
/// Mac's file is written.
pub fn tick_and_sync(state: &AppState, at: Moment, dir: Option<&Path>) -> Result<PetResponse, String> {
    let fetched = fetch_folder(state, false);
    run_tick(state, at, dir, SyncPass::Full(fetched))
}

/// SYNC NOW: reads every device file, even if none seems to have changed.
pub(crate) fn sync_now_at(state: &AppState, at: Moment, dir: Option<&Path>) -> Result<SyncStatusView, String> {
    let fetched = fetch_folder(state, true);
    run_tick(state, at, dir, SyncPass::Full(fetched))?;
    Ok(lock_sync(state)?.status_view())
}

enum SyncPass {
    /// Only the ledger: the poll every 5 seconds while the popover is open.
    Record,
    /// Merge (with what was read, if anything), write back and write the own file.
    Full(Option<FetchOutcome>),
}

/// Reads the folder with no game lock held, so a slow folder never holds up
/// the popover. None when sync is off or a read is already running.
fn fetch_folder(state: &AppState, force: bool) -> Option<FetchOutcome> {
    let job = state.sync.lock().ok()?.begin_fetch(force)?;
    Some(job.run(READ_TIMEOUT))
}

fn run_tick(state: &AppState, at: Moment, dir: Option<&Path>, pass: SyncPass) -> Result<PetResponse, String> {
    // pet → companion → stats → sync, as AppState requires
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;

    pet.update_at(at.now, at.idle_secs);
    companion.advance_day(&pet, at.today);
    let _ = achievements::check_achievements(&mut stats, &pet);
    let passive_before = stats.last_passive_time;
    let passive = stats.accrue_passive(at.now, pet.is_alive);

    {
        let mut sync = lock_sync(state)?;
        if sync.is_on() {
            sync.record_passive(passive_before, stats.last_passive_time, passive);
            match pass {
                SyncPass::Record => {
                    sync.observe_companion(&companion);
                    sync.observe_stats(&stats);
                    sync.persist_local();
                }
                SyncPass::Full(fetched) => {
                    if let Some(fetched) = fetched {
                        sync.finish_fetch(fetched, at.now);
                    }
                    sync.apply(&mut pet, &mut companion, &mut stats, at);
                    sync.write();
                }
            }
        }
    }

    if let Some(dir) = dir {
        log_save_error("pet state", storage::save_state_in(dir, &pet));
        log_save_error("companion", storage::save_companion_in(dir, &companion));
        log_save_error("game stats", storage::save_stats_in(dir, &stats));
    }

    Ok(PetResponse::from(&pet))
}

fn log_save_error(what: &str, result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("Failed to save {}: {}", what, e);
    }
}

/// The save directory for `tick`, or None (logged) when it is unavailable.
pub fn save_dir() -> Option<PathBuf> {
    storage::config_dir()
        .map_err(|e| eprintln!("Save directory unavailable: {}", e))
        .ok()
}

#[tauri::command]
pub fn get_pet_state(state: State<AppState>) -> Result<PetResponse, String> {
    tick(&state, Moment::current(), save_dir().as_deref())
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

/// NEW PET / NEW FRIEND?: a new egg with no Bond. Coins, achievements and
/// the streak carry over (R10).
fn revive(pet: &mut PetState, companion: &mut CompanionState, stats: &mut GameStats) -> Vec<Achievement> {
    crate::pet::lifecycle::revive(pet);
    companion.reset_for_new_pet();
    stats.record_revive();
    achievements::check_achievements(stats, pet)
}

pub(crate) fn revive_at(state: &AppState, dir: Option<&Path>) -> Result<ActionResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let mut sync = lock_sync(state)?;
    let synced = sync.is_on();
    if synced {
        // The last snapshot of the pet that is gone
        sync.observe_pet(&pet);
        // RESET on a living pet replaces it on purpose, so the other Macs
        // must not offer it back at WHICH DOT?. A dead one is not retired: care
        // given after its death on another Mac still keeps it (design マージ規則)
        if pet.is_alive {
            sync.retire_pet(&pet.id);
        }
    }
    let new_achievements = revive(&mut pet, &mut companion, &mut stats);
    if synced {
        // A new id: the old one stays dead in every device file, so a Mac
        // that has not heard of the death cannot take the egg for it
        pet.id = uuid::Uuid::new_v4().to_string();
        sync.record_revive();
        sync.observe_pet(&pet);
        sync.observe_stats(&stats);
        sync.write();
    }
    drop(sync);
    save_pet_in(dir, &pet);
    save_companion_in(dir, &companion);
    save_stats_in(dir, &stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn revive_pet(state: State<AppState>) -> Result<ActionResult, String> {
    revive_at(&state, save_dir().as_deref())
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
    let today = Local::now().format("%Y-%m-%d").to_string();
    let report = GameReport { token: &token, game: &game, outcome: &outcome, moves };
    report_game_result_at(&state, report, Instant::now(), &today, save_dir().as_deref())
}

/// What the UI reports when a mini game ends.
pub(crate) struct GameReport<'a> {
    pub token: &'a str,
    pub game: &'a str,
    pub outcome: &'a str,
    pub moves: Option<u32>,
}

/// `today` is the local date as `YYYY-MM-DD`, for the daily cap. The cap is
/// per Mac, also with sync on.
pub(crate) fn report_game_result_at(
    state: &AppState,
    report: GameReport,
    now: Instant,
    today: &str,
    dir: Option<&Path>,
) -> Result<RewardResult, String> {
    let mut sessions = state.game_sessions.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let reward = game_session::claim_reward(
        &mut sessions, &mut stats, report.token, report.game, report.outcome, report.moves, now, today,
    )?;
    {
        let mut sync = lock_sync(state)?;
        if sync.is_on() {
            sync.record_earned(reward.earned);
            sync.observe_stats(&stats);
            sync.write();
        }
    }
    save_stats_in(dir, &stats);
    Ok(RewardResult { earned: reward.earned, coins: stats.coins, capped: reward.capped })
}

pub(crate) fn buy_snack_at(state: &AppState, snack_id: &str, now: i64, dir: Option<&Path>) -> Result<ActionResult, String> {
    let snack = economy::find_snack(snack_id).ok_or_else(|| "unknown snack".to_string())?;

    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let mut sync = lock_sync(state)?;

    // With sync on, the coins of all Macs: a balance below zero buys nothing
    if stats.coins < snack.price || !sync.can_spend(snack.price, now) {
        return Err(format!("not enough coins ({} needed)", snack.price));
    }

    // Validate and apply to the pet first, so coins are only spent when the snack is eaten.
    pet.eat_snack(snack)?;
    stats.try_spend(snack.price);
    save_pet_in(dir, &pet);
    let new_achievements = achievements::check_achievements(&mut stats, &pet);
    if sync.is_on() {
        sync.record_spent(snack.price);
        sync.observe_pet(&pet);
        sync.observe_stats(&stats);
        sync.write();
    }
    drop(sync);
    save_stats_in(dir, &stats);

    Ok(ActionResult::from(&pet, new_achievements))
}

#[tauri::command]
pub fn buy_snack(state: State<AppState>, snack_id: String) -> Result<ActionResult, String> {
    buy_snack_at(&state, &snack_id, Utc::now().timestamp(), save_dir().as_deref())
}

/// What the popover shows when it opens. Bond is never part of it.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CompanionOpenResult {
    /// The pet is a living adult; otherwise ignore the rest.
    pub enabled: bool,
    pub mood: String,
    pub line: Option<SaidLine>,
    /// "prompt" to ask `STAY WITH YOU?`, otherwise "none".
    pub onboarding: String,
}

impl From<OpenResult> for CompanionOpenResult {
    fn from(result: OpenResult) -> Self {
        Self {
            enabled: result.enabled,
            mood: result.mood.as_str().to_string(),
            line: result.line,
            onboarding: if result.prompt { "prompt" } else { "none" }.to_string(),
        }
    }
}

/// Called once each time the popover opens (the window gains focus). With
/// sync on, the folder is read in the background; what it brings shows on
/// the next poll.
#[tauri::command]
pub fn companion_open(app: AppHandle, state: State<AppState>) -> Result<CompanionOpenResult, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    pet.update();
    save_pet(&pet);

    // After `pet`, as AppState requires
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let local = Local::now();
    let seed = local.timestamp_nanos_opt().unwrap_or_else(|| local.timestamp()) as u64;
    let result = companion::open(
        &mut companion,
        &pet,
        local.timestamp(),
        local.naive_local(),
        &mut StdRng::seed_from_u64(seed),
    );
    if result.enabled {
        save_companion(&companion);
    }
    let synced = {
        let mut sync = lock_sync(&state)?;
        if sync.is_on() {
            // The greeting counts on this Mac before any merge
            sync.observe_companion(&companion);
            sync.persist_local();
        }
        sync.is_on()
    };
    drop(companion);
    drop(pet);
    if synced {
        sync_in_background(app);
    }
    Ok(result.into())
}

/// `tick_and_sync` on a thread of its own, so the command returns at once.
fn sync_in_background(app: AppHandle) {
    let spawned = std::thread::Builder::new().name("dot-sync-open".into()).spawn(move || {
        let state = app.state::<AppState>();
        if let Err(e) = tick_and_sync(&state, Moment::current(), save_dir().as_deref()) {
            eprintln!("Sync on open failed: {}", e);
        }
    });
    if let Err(e) = spawned {
        eprintln!("Could not start the sync on open: {}", e);
    }
}

/// The mood while the popover stays open. `enabled` as in `CompanionOpenResult`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CompanionMoodResult {
    pub enabled: bool,
    pub mood: String,
}

impl From<Option<companion::Mood>> for CompanionMoodResult {
    fn from(mood: Option<companion::Mood>) -> Self {
        Self {
            enabled: mood.is_some(),
            mood: mood.unwrap_or(companion::Mood::Calm).as_str().to_string(),
        }
    }
}

/// Called with the regular poll while the popover is open. Not an open: no
/// visit is counted and no line is said.
#[tauri::command]
pub fn refresh_companion_mood(state: State<AppState>) -> Result<CompanionMoodResult, String> {
    // The poll has just updated the pet
    let pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let local = Local::now();
    let before = companion.mood;
    let mood = companion::open::refresh(&mut companion, &pet, local.timestamp(), local.hour());
    if companion.mood != before {
        save_companion(&companion);
    }
    Ok(mood.into())
}

#[tauri::command]
pub fn get_companion_settings(state: State<AppState>) -> Result<SettingsView, String> {
    let companion = state.companion.lock().map_err(|e| e.to_string())?;
    Ok(companion.settings.view(Utc::now().timestamp()))
}

/// Changes only the fields given; refuses the whole change if any is invalid.
#[tauri::command]
pub fn set_companion_settings(state: State<AppState>, update: SettingsUpdate) -> Result<SettingsView, String> {
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let local = Local::now();
    companion::settings::apply(&mut companion.settings, &update, local.timestamp(), local.naive_local())?;
    save_companion(&companion);
    Ok(companion.settings.view(local.timestamp()))
}

/// `answer` is "yes" or "later".
#[tauri::command]
pub fn companion_onboarding_answer(state: State<AppState>, answer: String) -> Result<(), String> {
    let answer = OnboardingAnswer::parse(&answer)?;
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    companion::settings::answer_onboarding(&mut companion.onboarding, answer, Utc::now().timestamp());
    save_companion(&companion);
    // YES on one Mac is YES on all of them
    let mut sync = lock_sync(&state)?;
    if sync.is_on() {
        sync.observe_companion(&companion);
        sync.write();
    }
    Ok(())
}

// --- SYNC (#38) ---

#[tauri::command]
pub fn get_sync_settings(state: State<AppState>) -> Result<SyncSettingsView, String> {
    Ok(lock_sync(&state)?.settings_view())
}

/// `path` is checked and saved first; an invalid one changes nothing. Then
/// `enabled` turns sync on or off. Turning it on reads the folder at once.
pub(crate) fn set_sync_settings_at(
    state: &AppState,
    path: Option<&str>,
    enabled: Option<bool>,
    at: Moment,
    dir: Option<&Path>,
) -> Result<SyncSettingsView, String> {
    let on = {
        let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
        let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
        let stats = state.stats.lock().map_err(|e| e.to_string())?;
        let mut sync = lock_sync(state)?;
        if let Some(path) = path {
            sync.set_folder(path)?;
        }
        match enabled {
            Some(true) => sync.enable(&mut pet, &mut companion, &stats)?,
            Some(false) => sync.disable(&stats)?,
            None => {}
        }
        sync.is_on()
    };
    if on {
        run_tick(state, at, dir, SyncPass::Full(fetch_folder(state, true)))?;
    }
    Ok(lock_sync(state)?.settings_view())
}

#[tauri::command(async)]
pub fn set_sync_settings(
    state: State<'_, AppState>,
    path: Option<String>,
    enabled: Option<bool>,
) -> Result<SyncSettingsView, String> {
    set_sync_settings_at(&state, path.as_deref(), enabled, Moment::current(), save_dir().as_deref())
}

#[tauri::command(async)]
pub fn sync_now(state: State<'_, AppState>) -> Result<SyncStatusView, String> {
    sync_now_at(&state, Moment::current(), save_dir().as_deref())
}

#[tauri::command]
pub fn get_sync_status(state: State<AppState>) -> Result<SyncStatusView, String> {
    Ok(lock_sync(&state)?.status_view())
}

/// `WHICH DOT?`: keeps `pet_id`, retires the other living pets.
pub(crate) fn choose_pet_at(state: &AppState, pet_id: &str, at: Moment, dir: Option<&Path>) -> Result<SyncStatusView, String> {
    let mut pet = state.pet.lock().map_err(|e| e.to_string())?;
    let mut companion = state.companion.lock().map_err(|e| e.to_string())?;
    let mut stats = state.stats.lock().map_err(|e| e.to_string())?;
    let mut sync = lock_sync(state)?;
    sync.choose(pet_id, &mut pet, &mut companion, &mut stats, at)?;
    sync.write();
    let status = sync.status_view();
    drop(sync);
    save_pet_in(dir, &pet);
    save_companion_in(dir, &companion);
    save_stats_in(dir, &stats);
    Ok(status)
}

#[tauri::command(async)]
pub fn sync_choose_pet(state: State<'_, AppState>, pet_id: String) -> Result<SyncStatusView, String> {
    choose_pet_at(&state, &pet_id, Moment::current(), save_dir().as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::EvolutionStage;

    #[test]
    fn open_result_reads_as_the_ui_expects() {
        let result = OpenResult {
            enabled: true,
            mood: companion::Mood::MissedYou,
            line: Some(SaidLine { id: "missed_1".to_string(), text: "WELCOME BACK.".to_string() }),
            prompt: false,
        };

        let json = serde_json::to_value(CompanionOpenResult::from(result)).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "enabled": true,
                "mood": "missed_you",
                "line": { "id": "missed_1", "text": "WELCOME BACK." },
                "onboarding": "none",
            })
        );
    }

    #[test]
    fn mood_result_reads_as_the_ui_expects() {
        let json = serde_json::to_value(CompanionMoodResult::from(Some(companion::Mood::Sleepy))).unwrap();
        assert_eq!(json, serde_json::json!({ "enabled": true, "mood": "sleepy" }));
        let off = CompanionMoodResult::from(None);
        assert!(!off.enabled);
    }

    #[test]
    fn a_prompt_reads_as_prompt() {
        let result = OpenResult { enabled: true, mood: companion::Mood::Calm, line: None, prompt: true };
        let ui = CompanionOpenResult::from(result);
        assert_eq!(ui.onboarding, "prompt");
        assert_eq!(ui.line, None);
    }

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
    fn new_friend_after_an_adult_died_starts_the_bond_over_but_keeps_coins_achievements_and_streak() {
        let (mut pet, mut companion) = adult();
        companion.record_open(&pet, now(), today());
        companion.record_care(&pet, 10, today());
        companion.bond = 420;
        companion.lines.recent_ids.push("greet_1".to_string());
        companion.onboarding = companion::Onboarding::Yes;
        pet.is_alive = false;
        let mut stats = GameStats::new();
        stats.add_coins(42);
        stats.unlocked.push("first_feed".to_string());
        stats.current_streak = 5;
        stats.longest_streak = 9;
        let raised = stats.total_pets_raised;

        revive(&mut pet, &mut companion, &mut stats);

        assert!(pet.is_alive);
        assert_eq!(pet.stage, EvolutionStage::Egg);
        assert_eq!(companion.bond, 0);
        assert_eq!(companion.today, Default::default());
        assert_eq!(companion.lines, Default::default());
        assert_eq!(companion.onboarding, companion::Onboarding::Unseen);
        assert_eq!(stats.coins, 42);
        assert!(stats.unlocked.contains(&"first_feed".to_string()));
        assert_eq!((stats.current_streak, stats.longest_streak), (5, 9));
        assert_eq!(stats.total_pets_raised, raised + 1);
    }

    // --- tick (P3) ---

    const T0: i64 = 1_790_000_000;

    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("dot-tick-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn present_at(now: i64) -> Moment {
        Moment { now, idle_secs: 0, today: today() }
    }

    /// A living adult at full needs, last updated and paid at T0.
    fn app_at_t0() -> AppState {
        let (mut pet, companion) = adult();
        pet.last_update = T0;
        pet.activity = Activity::Idle;
        pet.activity_until = None;
        let mut stats = GameStats::new();
        stats.last_passive_time = Some(T0);
        AppState::with(pet, stats, companion, None)
    }

    fn json<T: Serialize>(value: &T) -> serde_json::Value {
        serde_json::to_value(value).unwrap()
    }

    #[test]
    fn tick_does_what_the_poll_did() {
        let app = app_at_t0();
        app.pet.lock().unwrap().hunger = 50;
        let mut pet = app.pet.lock().unwrap().clone();
        let mut companion = app.companion.lock().unwrap().clone();
        let mut stats = app.stats.lock().unwrap().clone();
        let at = Moment { now: T0 + 1800, idle_secs: 120, today: today() };

        // The body of get_pet_state before the tick existed, with the same time and idle
        pet.update_at(at.now, at.idle_secs);
        companion.advance_day(&pet, at.today);
        let _ = achievements::check_achievements(&mut stats, &pet);
        stats.accrue_passive(at.now, pet.is_alive);

        let dir = TempDir::new();
        let response = tick(&app, at, Some(&dir.0)).unwrap();

        assert_eq!(json(&response), json(&PetResponse::from(&pet)));
        assert_eq!(json(&*app.pet.lock().unwrap()), json(&pet));
        assert_eq!(*app.companion.lock().unwrap(), companion);
        assert_eq!(json(&*app.stats.lock().unwrap()), json(&stats));
        // The adult unlocks its evolution achievement on the tick, as on the poll
        assert!(stats.unlocked.contains(&"all_grown_up".to_string()));
        assert_eq!(stats.coins, 3);
    }

    #[test]
    fn tick_saves_what_it_changed() {
        let app = app_at_t0();
        let dir = TempDir::new();

        tick(&app, present_at(T0 + 3600), Some(&dir.0)).unwrap();

        let loaded = AppState::load_from(&dir.0, today());
        assert_eq!(json(&*loaded.pet.lock().unwrap()), json(&*app.pet.lock().unwrap()));
        assert_eq!(json(&*loaded.stats.lock().unwrap()), json(&*app.stats.lock().unwrap()));
        assert_eq!(*loaded.companion.lock().unwrap(), *app.companion.lock().unwrap());
        assert_eq!(loaded.take_load_warning(), None);
    }

    #[test]
    fn ticks_while_hidden_keep_the_user_present_and_pay_coins() {
        let app = app_at_t0();
        let dir = TempDir::new();
        let mut expected = app.pet.lock().unwrap().clone();
        let mult = crate::pet::needs::stage_decay_mult(expected.stage);
        let hours = 3;

        let mut now = T0;
        for _ in 0..hours * 60 {
            now += SIGNAL_TICK_SECS as i64;
            tick(&app, present_at(now), Some(&dir.0)).unwrap();
            // (c) one coin per full 600 seconds so far
            assert_eq!(app.stats.lock().unwrap().coins, ((now - T0) / 600) as u64);
        }

        let pet = app.pet.lock().unwrap();
        assert!(pet.is_alive);
        // (a) every minute used the present rules: 3 h at Adult 0.2 is hunger 100 − 0.04×180,
        // happiness 100 − 0.03×180, energy 100 − 0.02×180 (92.8 / 94.6 / 96.4, rounded)
        crate::pet::needs::apply_decay(&mut expected, hours as u64 * 3600, mult);
        assert_eq!(
            [pet.hunger, pet.happiness, pet.energy, pet.health],
            [expected.hunger, expected.happiness, expected.energy, expected.health]
        );
        assert_eq!([pet.hunger, pet.happiness, pet.energy, pet.health], [93, 95, 96, 100]);
        // (b) deep sleep would have stopped happiness after 30 minutes (99 at most)
        assert!(pet.happiness < 99);
        assert_eq!(pet.age, PetState::new().age + (hours * 3600) as u64);
        assert_eq!(app.stats.lock().unwrap().coins, (hours * 6) as u64);
    }

    #[test]
    fn ticks_through_a_long_idle_match_one_update_over_the_same_span() {
        let app = app_at_t0();
        let mut once = app.pet.lock().unwrap().clone();
        let span = 3 * 3600;

        // The user stops typing at T0; each tick reads how long it has been
        let mut now = T0;
        while now < T0 + span {
            now += SIGNAL_TICK_SECS as i64;
            let at = Moment { now, idle_secs: (now - T0) as u64, today: today() };
            tick(&app, at, None).unwrap();
        }
        // The first 30 minutes present, the rest deep sleep, as F5 splits one long gap
        once.update_at(T0 + span, span as u64);

        let pet = app.pet.lock().unwrap();
        assert_eq!(
            [pet.hunger, pet.happiness, pet.energy, pet.health],
            [once.hunger, once.happiness, once.energy, once.health]
        );
        assert!(pet.is_alive);
    }

    #[test]
    fn ticks_for_a_dead_pet_change_nothing_and_pay_nothing() {
        let app = app_at_t0();
        app.pet.lock().unwrap().is_alive = false;
        app.stats.lock().unwrap().coins = 7;
        let dir = TempDir::new();
        let pet_before = json(&*app.pet.lock().unwrap());
        let companion_before = app.companion.lock().unwrap().clone();

        let mut now = T0;
        for _ in 0..120 {
            now += SIGNAL_TICK_SECS as i64;
            tick(&app, present_at(now), Some(&dir.0)).unwrap();
        }

        assert_eq!(json(&*app.pet.lock().unwrap()), pet_before);
        assert_eq!(*app.companion.lock().unwrap(), companion_before);
        assert_eq!(app.stats.lock().unwrap().coins, 7);
        let loaded = AppState::load_from(&dir.0, today());
        assert_eq!(loaded.take_load_warning(), None);
        assert!(!loaded.pet.lock().unwrap().is_alive);
        assert_eq!(loaded.stats.lock().unwrap().coins, 7);
    }

    #[test]
    fn tick_without_a_save_directory_still_updates() {
        let app = app_at_t0();

        tick(&app, present_at(T0 + 600), None).unwrap();

        assert_eq!(app.pet.lock().unwrap().last_update, T0 + 600);
        assert_eq!(app.stats.lock().unwrap().coins, 1);
    }

    #[test]
    fn tick_reports_a_poisoned_lock_instead_of_panicking() {
        let app = std::sync::Arc::new(app_at_t0());
        let poisoner = std::sync::Arc::clone(&app);
        let _ = std::thread::spawn(move || {
            let _guard = poisoner.stats.lock().unwrap();
            panic!("poison the stats lock");
        })
        .join();

        assert!(tick(&app, present_at(T0 + 600), None).is_err());
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

//! Server-side checks for mini game rewards: a game must be started with a
//! one-shot token, played for a minimum time, and its coins count towards a
//! daily cap. Pending sessions live in memory only (a restart drops them).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use rand::{rngs::StdRng, SeedableRng};

use crate::minigames::{GameState, StepInput, StepResult};
use crate::pet::{economy, GameStats};

/// Oldest pending sessions are dropped beyond this many.
pub const MAX_PENDING: usize = 8;
/// A session older than this can no longer be reported.
pub const SESSION_TTL: Duration = Duration::from_secs(3600);
/// 8 cards = 4 pairs, so a memory clear takes at least 4 moves.
pub const MEMORY_MIN_MOVES: u32 = 4;

/// Shortest believable time from `start_game` to the report, or `None` for an unknown game.
pub fn min_play_time(game: &str, moves: Option<u32>) -> Option<Duration> {
    match game {
        "janken" => Some(Duration::from_secs(1)),
        "blackjack" => Some(Duration::from_secs(3)),
        "memory" => {
            let by_moves = Duration::from_millis(500) * moves.unwrap_or(0);
            Some(by_moves.max(Duration::from_secs(4)))
        }
        // A second per guess: 3 digits to tap, then OK
        "hit_blow" => Some((Duration::from_secs(1) * moves.unwrap_or(0)).max(Duration::from_secs(3))),
        _ => None,
    }
}

struct PendingGame {
    game: String,
    started_at: Instant,
    /// For a game played through `step`: its answer and progress
    state: Option<GameState>,
}

pub struct GameSessions {
    pending: HashMap<String, PendingGame>,
    /// Deals the answers of the games played through `step`
    rng: StdRng,
}

impl Default for GameSessions {
    fn default() -> Self {
        Self { pending: HashMap::new(), rng: StdRng::from_os_rng() }
    }
}

impl GameSessions {
    pub fn new() -> Self {
        Self::default()
    }

    /// The same seed deals the same answers, so tests can replay a game.
    pub fn with_seed(seed: u64) -> Self {
        Self { pending: HashMap::new(), rng: StdRng::seed_from_u64(seed) }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Opens a session for `game` and returns its token.
    pub fn start(&mut self, game: &str, now: Instant) -> Result<String, String> {
        if min_play_time(game, None).is_none() {
            return Err(format!("unknown game: {game}"));
        }
        self.pending
            .retain(|_, p| now.saturating_duration_since(p.started_at) <= SESSION_TTL);
        while self.pending.len() >= MAX_PENDING {
            let Some(oldest) = self
                .pending
                .iter()
                .min_by_key(|(_, p)| p.started_at)
                .map(|(token, _)| token.clone())
            else {
                break;
            };
            self.pending.remove(&oldest);
        }

        let token = uuid::Uuid::new_v4().to_string();
        let state = GameState::new(game, &mut self.rng);
        self.pending.insert(
            token.clone(),
            PendingGame { game: game.to_string(), started_at: now, state },
        );
        Ok(token)
    }

    /// Plays one move of a game that keeps its answer here. The token stays
    /// open until `finish`.
    pub fn step(&mut self, token: &str, input: &StepInput, now: Instant) -> Result<StepResult, String> {
        let session = self
            .pending
            .get_mut(token)
            .ok_or_else(|| "unknown or used game token".to_string())?;
        if now.saturating_duration_since(session.started_at) > SESSION_TTL {
            self.pending.remove(token);
            return Err("game token expired".to_string());
        }
        let state = session
            .state
            .as_mut()
            .ok_or_else(|| format!("{} has no steps", session.game))?;
        state.step(input)
    }

    /// Consumes the token (whether or not the check passes) and checks the report against it.
    /// Returns the coins for a game that keeps its answer here, decided by its
    /// state; `None` for a game whose reported outcome sets the coins.
    pub fn finish(&mut self, token: &str, game: &str, moves: Option<u32>, now: Instant) -> Result<Option<u64>, String> {
        let session = self
            .pending
            .remove(token)
            .ok_or_else(|| "unknown or used game token".to_string())?;
        let played = now.saturating_duration_since(session.started_at);
        if played > SESSION_TTL {
            return Err("game token expired".to_string());
        }
        if session.game != game {
            return Err(format!("token was issued for {}, not {game}", session.game));
        }
        // The reported moves are ignored here: the state counted them
        let (moves, reward) = match &session.state {
            Some(state) => {
                let settled = state.settled().ok_or_else(|| format!("{game} is not over yet"))?;
                (Some(settled.moves), Some(settled.reward))
            }
            None => (moves, None),
        };
        if game == "memory" && moves.unwrap_or(0) < MEMORY_MIN_MOVES {
            return Err(format!("memory needs at least {MEMORY_MIN_MOVES} moves"));
        }
        let min = min_play_time(game, moves).ok_or_else(|| format!("unknown game: {game}"))?;
        if played < min {
            return Err("game finished too quickly".to_string());
        }
        Ok(reward)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameReward {
    pub earned: u64,
    /// The daily cap cut this reward (fully or partly).
    pub capped: bool,
}

/// Validates a finished game and pays its coins, up to the daily cap.
/// For a game played through `GameSessions::step`, `outcome` and `moves` are
/// ignored: its state decides the coins.
/// `today` is the local date as `YYYY-MM-DD`.
#[allow(clippy::too_many_arguments)]
pub fn claim_reward(
    sessions: &mut GameSessions,
    stats: &mut GameStats,
    token: &str,
    game: &str,
    outcome: &str,
    moves: Option<u32>,
    now: Instant,
    today: &str,
) -> Result<GameReward, String> {
    let reward = match sessions.finish(token, game, moves, now)? {
        Some(decided) => decided,
        None => economy::game_reward(game, outcome, moves)?,
    };
    let earned = stats.add_game_coins(reward, today, economy::DAILY_GAME_COIN_CAP);
    Ok(GameReward { earned, capped: earned < reward })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minigames::hit_blow::HitBlow;
    use crate::minigames::{StepInput, View as MiniView};
    use crate::pet::economy::DAILY_GAME_COIN_CAP;

    const DAY: &str = "2026-10-14";

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    fn claim(
        sessions: &mut GameSessions,
        stats: &mut GameStats,
        token: &str,
        game: &str,
        outcome: &str,
        moves: Option<u32>,
        now: Instant,
    ) -> Result<GameReward, String> {
        claim_reward(sessions, stats, token, game, outcome, moves, now, DAY)
    }

    #[test]
    fn started_game_reported_in_time_pays_coins() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("blackjack", t0).unwrap();

        let r = claim(&mut sessions, &mut stats, &token, "blackjack", "win", None, t0 + secs(3)).unwrap();

        assert_eq!(r, GameReward { earned: 5, capped: false });
        assert_eq!(stats.coins, 5);
    }

    #[test]
    fn token_pays_only_once() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("janken", t0).unwrap();
        claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + secs(2)).unwrap();

        let again = claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + secs(4));

        assert!(again.is_err());
        assert_eq!(stats.coins, 2);
    }

    #[test]
    fn tokens_are_unique() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let a = sessions.start("janken", t0).unwrap();
        let b = sessions.start("janken", t0).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn unknown_game_cannot_start() {
        let mut sessions = GameSessions::new();
        assert!(sessions.start("poker", Instant::now()).is_err());
        assert_eq!(sessions.pending_count(), 0);
    }

    #[test]
    fn missing_or_unknown_token_pays_nothing() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        sessions.start("janken", t0).unwrap();

        for token in ["", "not-a-token"] {
            let r = claim(&mut sessions, &mut stats, token, "janken", "win", None, t0 + secs(5));
            assert!(r.is_err(), "token {token:?} was accepted");
        }
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn expired_token_pays_nothing() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("janken", t0).unwrap();

        let r = claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + SESSION_TTL + secs(1));

        assert!(r.is_err());
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn token_for_another_game_pays_nothing() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("janken", t0).unwrap();

        let r = claim(&mut sessions, &mut stats, &token, "memory", "clear", Some(4), t0 + secs(60));

        assert!(r.is_err());
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn report_faster_than_minimum_pays_nothing() {
        let t0 = Instant::now();
        let cases = [
            ("janken", "win", None, Duration::from_millis(999)),
            ("blackjack", "blackjack", None, Duration::from_millis(2_999)),
            ("memory", "clear", Some(4), Duration::from_millis(3_999)), // 4 s floor
            ("memory", "clear", Some(10), Duration::from_millis(4_999)), // 10 x 0.5 s
        ];
        for (game, outcome, moves, elapsed) in cases {
            let mut sessions = GameSessions::new();
            let mut stats = GameStats::new();
            let token = sessions.start(game, t0).unwrap();

            let r = claim(&mut sessions, &mut stats, &token, game, outcome, moves, t0 + elapsed);

            assert!(r.is_err(), "{game} after {elapsed:?} was accepted");
            assert_eq!(stats.coins, 0);
        }
    }

    #[test]
    fn rejected_report_still_uses_up_the_token() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("janken", t0).unwrap();
        assert!(claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0).is_err());

        let later = claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + secs(5));

        assert!(later.is_err());
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn memory_with_fewer_than_four_moves_pays_nothing() {
        let t0 = Instant::now();
        for moves in [None, Some(0), Some(3)] {
            let mut sessions = GameSessions::new();
            let mut stats = GameStats::new();
            let token = sessions.start("memory", t0).unwrap();

            let r = claim(&mut sessions, &mut stats, &token, "memory", "clear", moves, t0 + secs(60));

            assert!(r.is_err(), "memory with {moves:?} moves was accepted");
            assert_eq!(stats.coins, 0);
        }
    }

    #[test]
    fn a_step_needs_a_live_token() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        for token in ["", "not-a-token"] {
            assert!(sessions.step(token, &StepInput::Look, t0).is_err(), "token {token:?} was accepted");
        }
    }

    #[test]
    fn games_the_ui_plays_on_its_own_have_no_steps() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let token = sessions.start("janken", t0).unwrap();

        assert!(sessions.step(&token, &StepInput::Look, t0 + secs(1)).is_err());
        // The token still pays as before
        let mut stats = GameStats::new();
        let r = claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + secs(2)).unwrap();
        assert_eq!(r.earned, 2);
    }

    /// Starts `game` and swaps in a state with a known answer.
    fn start_with(sessions: &mut GameSessions, game: &str, state: GameState, t0: Instant) -> String {
        let token = sessions.start(game, t0).unwrap();
        sessions.pending.get_mut(&token).unwrap().state = Some(state);
        token
    }

    fn guess(digits: [u8; 3]) -> StepInput {
        StepInput::Guess { digits: digits.to_vec() }
    }

    fn hit_blow(sessions: &mut GameSessions, answer: [u8; 3], t0: Instant) -> String {
        start_with(sessions, "hit_blow", GameState::HitBlow(HitBlow::with_answer(answer)), t0)
    }

    #[test]
    fn hit_blow_pays_by_its_own_count_whatever_the_ui_reports() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let mut stats = GameStats::new();
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);
        sessions.step(&token, &guess([4, 5, 6]), t0 + secs(1)).unwrap();
        let last = sessions.step(&token, &guess([1, 2, 3]), t0 + secs(2)).unwrap();
        assert!(last.finished);
        assert_eq!((last.moves, last.reward), (Some(2), Some(7)));

        // The UI claims a first-try clear; the state says 2 guesses
        let r = claim(&mut sessions, &mut stats, &token, "hit_blow", "clear", Some(1), t0 + secs(5)).unwrap();

        assert_eq!(r, GameReward { earned: 7, capped: false });
        assert_eq!(stats.coins, 7);
    }

    #[test]
    fn hit_blow_unfinished_pays_nothing_and_uses_up_the_token() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let mut stats = GameStats::new();
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);
        sessions.step(&token, &guess([4, 5, 6]), t0 + secs(1)).unwrap();

        assert!(claim(&mut sessions, &mut stats, &token, "hit_blow", "clear", Some(1), t0 + secs(30)).is_err());
        assert_eq!(stats.coins, 0);
        assert!(sessions.step(&token, &guess([1, 2, 3]), t0 + secs(31)).is_err());
        assert!(claim(&mut sessions, &mut stats, &token, "hit_blow", "clear", None, t0 + secs(32)).is_err());
    }

    #[test]
    fn hit_blow_failed_game_pays_nothing() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let mut stats = GameStats::new();
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);
        for _ in 0..8 {
            sessions.step(&token, &guess([4, 5, 6]), t0).unwrap();
        }

        let r = claim(&mut sessions, &mut stats, &token, "hit_blow", "clear", Some(1), t0 + secs(8)).unwrap();

        assert_eq!(r, GameReward { earned: 0, capped: false });
    }

    #[test]
    fn hit_blow_bad_guess_is_refused_without_using_a_try() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);

        assert!(sessions.step(&token, &guess([1, 1, 2]), t0).is_err());
        assert!(sessions.step(&token, &StepInput::Guess { digits: vec![1, 2] }, t0).is_err());

        let look = sessions.step(&token, &StepInput::Look, t0).unwrap();
        let MiniView::HitBlow(view) = look.view else { panic!("not hit & blow") };
        assert_eq!(view.left, 8);
    }

    #[test]
    fn hit_blow_never_shows_the_answer_before_the_end() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let token = hit_blow(&mut sessions, [7, 8, 9], t0);
        for input in [StepInput::Look, guess([1, 2, 3]), guess([7, 9, 8])] {
            let json = serde_json::to_value(sessions.step(&token, &input, t0).unwrap()).unwrap();
            assert_eq!(json["view"]["answer"], serde_json::Value::Null, "{json}");
            assert_eq!(json["finished"], false);
            assert_eq!(json["reward"], serde_json::Value::Null);
        }
        let end = serde_json::to_value(sessions.step(&token, &guess([7, 8, 9]), t0).unwrap()).unwrap();
        assert_eq!(end["view"]["answer"], serde_json::json!([7, 8, 9]));
        assert!(sessions.step(&token, &guess([7, 8, 9]), t0).is_err(), "a move after the end");
    }

    #[test]
    fn hit_blow_is_replayed_by_the_same_seed() {
        let t0 = Instant::now();
        let play = |seed| {
            let mut sessions = GameSessions::with_seed(seed);
            let token = sessions.start("hit_blow", t0).unwrap();
            [[0, 1, 2], [3, 4, 5], [6, 7, 8], [9, 0, 1]]
                .map(|d| sessions.step(&token, &guess(d), t0).unwrap())
        };
        assert_eq!(play(3), play(3));
    }

    #[test]
    fn hit_blow_must_take_a_second_per_guess() {
        let t0 = Instant::now();
        // Solved on the 1st guess: 3 s floor; on the 5th: 5 x 1 s
        for (wrong, min) in [(0, secs(3)), (4, secs(5))] {
            let mut sessions = GameSessions::with_seed(1);
            let mut stats = GameStats::new();
            let token = hit_blow(&mut sessions, [1, 2, 3], t0);
            for _ in 0..wrong {
                sessions.step(&token, &guess([4, 5, 6]), t0).unwrap();
            }
            sessions.step(&token, &guess([1, 2, 3]), t0).unwrap();
            let early = min - Duration::from_millis(1);
            assert!(claim(&mut sessions, &mut stats, &token, "hit_blow", "x", None, t0 + early).is_err());

            let token = hit_blow(&mut sessions, [1, 2, 3], t0);
            for _ in 0..wrong {
                sessions.step(&token, &guess([4, 5, 6]), t0).unwrap();
            }
            sessions.step(&token, &guess([1, 2, 3]), t0).unwrap();
            assert!(claim(&mut sessions, &mut stats, &token, "hit_blow", "x", None, t0 + min).is_ok());
        }
    }

    #[test]
    fn hit_blow_near_the_cap_pays_only_the_remainder() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let mut stats = GameStats::new();
        stats.game_coins_today = 28;
        stats.game_coins_date = Some(DAY.to_string());
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);
        sessions.step(&token, &guess([1, 2, 3]), t0).unwrap();

        let r = claim(&mut sessions, &mut stats, &token, "hit_blow", "x", None, t0 + secs(3)).unwrap();

        assert_eq!(r, GameReward { earned: 2, capped: true });
        assert_eq!(stats.game_coins_today, DAILY_GAME_COIN_CAP);
    }

    #[test]
    fn a_used_token_has_no_more_steps() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let mut stats = GameStats::new();
        let token = hit_blow(&mut sessions, [1, 2, 3], t0);
        sessions.step(&token, &guess([1, 2, 3]), t0).unwrap();
        claim(&mut sessions, &mut stats, &token, "hit_blow", "x", None, t0 + secs(3)).unwrap();

        assert!(sessions.step(&token, &StepInput::Look, t0 + secs(4)).is_err());
        assert!(claim(&mut sessions, &mut stats, &token, "hit_blow", "x", None, t0 + secs(5)).is_err());
        assert_eq!(stats.coins, 8);
    }

    #[test]
    fn an_expired_token_has_no_more_steps() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::with_seed(1);
        let token = sessions.start("hit_blow", t0).unwrap();
        assert!(sessions.step(&token, &StepInput::Look, t0 + SESSION_TTL + secs(1)).is_err());
        assert_eq!(sessions.pending_count(), 0);
    }

    #[test]
    fn minimum_play_times() {
        assert_eq!(min_play_time("janken", None), Some(secs(1)));
        assert_eq!(min_play_time("blackjack", None), Some(secs(3)));
        assert_eq!(min_play_time("memory", Some(4)), Some(secs(4)));
        assert_eq!(min_play_time("memory", Some(8)), Some(secs(4)));
        assert_eq!(min_play_time("memory", Some(9)), Some(Duration::from_millis(4_500)));
        assert_eq!(min_play_time("hit_blow", Some(1)), Some(secs(3)));
        assert_eq!(min_play_time("hit_blow", Some(3)), Some(secs(3)));
        assert_eq!(min_play_time("hit_blow", Some(8)), Some(secs(8)));
        assert_eq!(min_play_time("poker", None), None);
    }

    #[test]
    fn pending_sessions_are_bounded_and_oldest_dropped() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let first = sessions.start("janken", t0).unwrap();
        for i in 1..=MAX_PENDING as u64 {
            sessions.start("janken", t0 + secs(i)).unwrap();
        }

        assert_eq!(sessions.pending_count(), MAX_PENDING);
        assert!(sessions.finish(&first, "janken", None, t0 + secs(60)).is_err());
    }

    #[test]
    fn expired_sessions_are_pruned_on_start() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        sessions.start("janken", t0).unwrap();
        sessions.start("janken", t0 + SESSION_TTL + secs(1)).unwrap();
        assert_eq!(sessions.pending_count(), 1);
    }

    #[test]
    fn invalid_outcome_pays_nothing() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        let token = sessions.start("janken", t0).unwrap();

        assert!(claim(&mut sessions, &mut stats, &token, "janken", "cheat", None, t0 + secs(5)).is_err());
        assert_eq!(stats.coins, 0);
        // The token is spent even though the outcome was rejected.
        assert!(claim(&mut sessions, &mut stats, &token, "janken", "win", None, t0 + secs(6)).is_err());
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn separate_games_share_one_daily_cap() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.game_coins_today = 25;
        stats.game_coins_date = Some(DAY.to_string());
        let a = sessions.start("blackjack", t0).unwrap();
        let b = sessions.start("memory", t0).unwrap();

        let first = claim(&mut sessions, &mut stats, &a, "blackjack", "win", None, t0 + secs(3)).unwrap();
        let second = claim(&mut sessions, &mut stats, &b, "memory", "clear", Some(4), t0 + secs(5)).unwrap();

        assert_eq!(first, GameReward { earned: 5, capped: false });
        assert_eq!(second, GameReward { earned: 0, capped: true });
        assert_eq!(stats.coins, 5);
    }

    fn play(sessions: &mut GameSessions, stats: &mut GameStats, today: &str) -> GameReward {
        let t0 = Instant::now();
        let token = sessions.start("blackjack", t0).unwrap();
        claim_reward(sessions, stats, &token, "blackjack", "win", None, t0 + secs(3), today).unwrap()
    }

    #[test]
    fn daily_cap_stops_game_coins_at_30() {
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        for _ in 0..6 {
            assert_eq!(play(&mut sessions, &mut stats, DAY), GameReward { earned: 5, capped: false });
        }
        assert_eq!(stats.coins, DAILY_GAME_COIN_CAP);

        assert_eq!(play(&mut sessions, &mut stats, DAY), GameReward { earned: 0, capped: true });
        assert_eq!(stats.coins, DAILY_GAME_COIN_CAP);
        assert_eq!(stats.game_coins_today, DAILY_GAME_COIN_CAP);
    }

    #[test]
    fn reward_near_the_cap_pays_only_the_remainder() {
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.game_coins_today = 28;
        stats.game_coins_date = Some(DAY.to_string());

        assert_eq!(play(&mut sessions, &mut stats, DAY), GameReward { earned: 2, capped: true });
        assert_eq!(stats.coins, 2);
        assert_eq!(stats.game_coins_today, 30);
    }

    #[test]
    fn daily_cap_resets_on_a_new_day() {
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.game_coins_today = DAILY_GAME_COIN_CAP;
        stats.game_coins_date = Some("2026-10-13".to_string());

        assert_eq!(play(&mut sessions, &mut stats, DAY), GameReward { earned: 5, capped: false });
        assert_eq!(stats.game_coins_today, 5);
        assert_eq!(stats.game_coins_date.as_deref(), Some(DAY));
    }

    #[test]
    fn daily_cap_does_not_reset_when_the_clock_goes_back() {
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.game_coins_today = DAILY_GAME_COIN_CAP;
        stats.game_coins_date = Some(DAY.to_string());

        assert_eq!(play(&mut sessions, &mut stats, "2026-10-13"), GameReward { earned: 0, capped: true });
        assert_eq!(stats.game_coins_date.as_deref(), Some(DAY));
    }

    #[test]
    fn zero_reward_at_the_cap_is_not_reported_as_capped() {
        let t0 = Instant::now();
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.game_coins_today = DAILY_GAME_COIN_CAP;
        stats.game_coins_date = Some(DAY.to_string());
        let token = sessions.start("janken", t0).unwrap();

        let r = claim(&mut sessions, &mut stats, &token, "janken", "lose", None, t0 + secs(2)).unwrap();

        assert_eq!(r, GameReward { earned: 0, capped: false });
    }

    #[test]
    fn other_income_is_not_counted_against_the_cap() {
        let mut sessions = GameSessions::new();
        let mut stats = GameStats::new();
        stats.add_coins(100); // passive / streak income
        assert_eq!(play(&mut sessions, &mut stats, DAY), GameReward { earned: 5, capped: false });
        assert_eq!(stats.game_coins_today, 5);
    }
}

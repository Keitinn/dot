//! Server-side checks for mini game rewards: a game must be started with a
//! one-shot token, played for a minimum time, and its coins count towards a
//! daily cap. Pending sessions live in memory only (a restart drops them).

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::pet::{economy, GameStats};

/// Oldest pending sessions are dropped beyond this many.
pub const MAX_PENDING: usize = 8;
/// A session older than this can no longer be reported.
pub const SESSION_TTL: Duration = Duration::from_secs(3600);
/// 8 cards = 4 pairs, so a memory clear takes at least 4 moves.
pub const MEMORY_MIN_MOVES: u32 = 4;

/// Shortest believable time from `start_game` to the report, or `None` for an unknown game.
pub fn min_play_time(game: &str, moves: Option<u32>) -> Option<Duration> {
    todo!("{game} {moves:?}")
}

struct PendingGame {
    game: String,
    started_at: Instant,
}

#[derive(Default)]
pub struct GameSessions {
    pending: HashMap<String, PendingGame>,
}

impl GameSessions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Opens a session for `game` and returns its token.
    pub fn start(&mut self, game: &str, now: Instant) -> Result<String, String> {
        todo!("{game} {now:?}")
    }

    /// Consumes the token (whether or not the check passes) and checks the report against it.
    pub fn finish(&mut self, token: &str, game: &str, moves: Option<u32>, now: Instant) -> Result<(), String> {
        todo!("{token} {game} {moves:?} {now:?}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameReward {
    pub earned: u64,
    /// The daily cap cut this reward (fully or partly).
    pub capped: bool,
}

/// Validates a finished game and pays its coins, up to the daily cap.
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
    todo!("{token} {game} {outcome} {moves:?} {now:?} {today} {} {}", sessions.pending_count(), stats.coins)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn minimum_play_times() {
        assert_eq!(min_play_time("janken", None), Some(secs(1)));
        assert_eq!(min_play_time("blackjack", None), Some(secs(3)));
        assert_eq!(min_play_time("memory", Some(4)), Some(secs(4)));
        assert_eq!(min_play_time("memory", Some(8)), Some(secs(4)));
        assert_eq!(min_play_time("memory", Some(9)), Some(Duration::from_millis(4_500)));
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

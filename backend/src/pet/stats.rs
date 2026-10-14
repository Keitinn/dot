use serde::{Deserialize, Serialize};
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStats {
    // Lifetime counters
    pub total_feeds: u64,
    pub total_plays: u64,
    pub total_sleeps: u64,
    pub total_pets_raised: u64,

    // Streak tracking
    pub current_streak: u32,
    pub longest_streak: u32,
    pub last_care_date: Option<String>, // "YYYY-MM-DD"

    // Achievement unlocks (store achievement IDs)
    pub unlocked: Vec<String>,

    // Economy (kept across pets: reviving does not reset coins)
    #[serde(default)]
    pub coins: u64,
    #[serde(default)]
    pub total_coins_earned: u64,
    #[serde(default)]
    pub last_passive_time: Option<i64>,

    // Mini game coins paid on `game_coins_date` (local "YYYY-MM-DD"), for the daily cap
    #[serde(default)]
    pub game_coins_today: u64,
    #[serde(default)]
    pub game_coins_date: Option<String>,
}

/// One coin every 10 minutes while the pet is alive.
pub const PASSIVE_INTERVAL_SECS: i64 = 600;
/// Passive income is paid for at most this long after the last payout
/// (so a laptop left closed for days does not pay out days of income).
pub const PASSIVE_MAX_SECS: i64 = 12 * 3600;
const PASSIVE_COINS_PER_INTERVAL: u64 = 1;

impl GameStats {
    pub fn new() -> Self {
        Self {
            total_feeds: 0,
            total_plays: 0,
            total_sleeps: 0,
            total_pets_raised: 1, // starts at 1 for first pet
            current_streak: 0,
            longest_streak: 0,
            last_care_date: None,
            unlocked: Vec::new(),
            coins: 0,
            total_coins_earned: 0,
            last_passive_time: None,
            game_coins_today: 0,
            game_coins_date: None,
        }
    }

    pub fn add_coins(&mut self, amount: u64) {
        self.coins = self.coins.saturating_add(amount);
        self.total_coins_earned = self.total_coins_earned.saturating_add(amount);
    }

    /// Returns false (and changes nothing) if there are not enough coins.
    pub fn try_spend(&mut self, amount: u64) -> bool {
        if self.coins < amount {
            return false;
        }
        self.coins -= amount;
        true
    }

    /// Pays passive income for the time since the last payout. Returns coins earned.
    /// Leftover time (less than one interval) carries over to the next call.
    pub fn accrue_passive(&mut self, now: i64, alive: bool) -> u64 {
        let last = match self.last_passive_time {
            Some(l) => l,
            None => {
                self.last_passive_time = Some(now); // no back-pay on first run
                return 0;
            }
        };
        if !alive || now < last {
            self.last_passive_time = Some(now);
            return 0;
        }
        let elapsed = now - last;
        if elapsed < PASSIVE_INTERVAL_SECS {
            return 0;
        }
        let paid_secs = elapsed.min(PASSIVE_MAX_SECS);
        let intervals = (paid_secs / PASSIVE_INTERVAL_SECS) as u64;
        let earned = intervals * PASSIVE_COINS_PER_INTERVAL;
        if elapsed > PASSIVE_MAX_SECS {
            self.last_passive_time = Some(now);
        } else {
            self.last_passive_time = Some(last + intervals as i64 * PASSIVE_INTERVAL_SECS);
        }
        self.add_coins(earned);
        earned
    }

    pub fn record_feed(&mut self) {
        self.total_feeds += 1;
        self.update_streak();
    }

    pub fn record_play(&mut self) {
        self.total_plays += 1;
        self.update_streak();
    }

    pub fn record_sleep(&mut self) {
        self.total_sleeps += 1;
        self.update_streak();
    }

    pub fn record_revive(&mut self) {
        self.total_pets_raised += 1;
    }

    fn update_streak(&mut self) {
        let today = Utc::now().format("%Y-%m-%d").to_string();

        match &self.last_care_date {
            Some(last_date) => {
                if *last_date == today {
                    // Already cared today, no streak change
                    return;
                }

                // Check if yesterday
                let yesterday = (Utc::now() - chrono::Duration::days(1))
                    .format("%Y-%m-%d")
                    .to_string();

                if *last_date == yesterday {
                    // Consecutive day - increment streak
                    self.current_streak += 1;
                } else {
                    // Gap in care - reset streak
                    self.current_streak = 1;
                }
            }
            None => {
                // First care ever
                self.current_streak = 1;
            }
        }

        // Daily care bonus: 1 coin per streak day, up to 10
        self.add_coins(self.current_streak.min(10) as u64);

        self.last_care_date = Some(today);
        if self.current_streak > self.longest_streak {
            self.longest_streak = self.current_streak;
        }
    }

    pub fn unlock_achievement(&mut self, id: &str) -> bool {
        if self.unlocked.contains(&id.to_string()) {
            return false;
        }
        self.unlocked.push(id.to_string());
        true
    }
}

impl Default for GameStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_stats() {
        let stats = GameStats::new();
        assert_eq!(stats.total_feeds, 0);
        assert_eq!(stats.total_plays, 0);
        assert_eq!(stats.total_sleeps, 0);
        assert_eq!(stats.total_pets_raised, 1);
        assert_eq!(stats.current_streak, 0);
        assert_eq!(stats.longest_streak, 0);
        assert!(stats.last_care_date.is_none());
        assert!(stats.unlocked.is_empty());
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn test_old_save_without_coins_loads() {
        let json = r#"{"total_feeds":3,"total_plays":1,"total_sleeps":0,"total_pets_raised":1,
            "current_streak":2,"longest_streak":2,"last_care_date":"2026-10-01","unlocked":[]}"#;
        let stats: GameStats = serde_json::from_str(json).unwrap();
        assert_eq!(stats.coins, 0);
        assert!(stats.last_passive_time.is_none());
        assert_eq!(stats.game_coins_today, 0);
        assert!(stats.game_coins_date.is_none());
    }

    #[test]
    fn test_spend_and_earn() {
        let mut stats = GameStats::new();
        assert!(!stats.try_spend(1)); // nothing to spend at 0
        stats.add_coins(10);
        assert!(stats.try_spend(4));
        assert_eq!(stats.coins, 6);
        assert!(!stats.try_spend(7));
        assert_eq!(stats.coins, 6); // unchanged on failure
        assert_eq!(stats.total_coins_earned, 10); // spending does not reduce lifetime earnings
    }

    #[test]
    fn test_first_care_gives_streak_bonus_once_per_day() {
        let mut stats = GameStats::new();
        stats.record_feed();
        assert_eq!(stats.coins, 1);
        stats.record_play(); // same day
        assert_eq!(stats.coins, 1);
    }

    #[test]
    fn test_streak_bonus_capped_at_10() {
        let mut stats = GameStats::new();
        let yesterday = (Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
        stats.last_care_date = Some(yesterday);
        stats.current_streak = 30;
        stats.record_feed();
        assert_eq!(stats.current_streak, 31);
        assert_eq!(stats.coins, 10);
    }

    #[test]
    fn test_passive_first_call_only_starts_clock() {
        let mut stats = GameStats::new();
        assert_eq!(stats.accrue_passive(1_000, true), 0);
        assert_eq!(stats.last_passive_time, Some(1_000));
        assert_eq!(stats.coins, 0);
    }

    #[test]
    fn test_passive_pays_per_interval_and_carries_remainder() {
        let mut stats = GameStats::new();
        stats.accrue_passive(0, true);
        assert_eq!(stats.accrue_passive(599, true), 0);
        assert_eq!(stats.accrue_passive(1_500, true), 2); // 2 intervals, 300s left over
        assert_eq!(stats.last_passive_time, Some(1_200));
        assert_eq!(stats.accrue_passive(1_800, true), 1); // remainder completes the next one
        assert_eq!(stats.coins, 3);
    }

    #[test]
    fn test_passive_capped_for_long_absence() {
        let mut stats = GameStats::new();
        stats.accrue_passive(0, true);
        let earned = stats.accrue_passive(10 * 24 * 3600, true);
        assert_eq!(earned, (PASSIVE_MAX_SECS / PASSIVE_INTERVAL_SECS) as u64); // 72
        assert_eq!(stats.last_passive_time, Some(10 * 24 * 3600)); // rest is forfeited
    }

    #[test]
    fn test_passive_not_paid_while_dead_and_survives_clock_rewind() {
        let mut stats = GameStats::new();
        stats.accrue_passive(0, true);
        assert_eq!(stats.accrue_passive(6_000, false), 0);
        assert_eq!(stats.last_passive_time, Some(6_000)); // dead time is not back-paid
        assert_eq!(stats.accrue_passive(5_000, true), 0); // clock moved backwards
        assert_eq!(stats.last_passive_time, Some(5_000));
        assert_eq!(stats.accrue_passive(5_600, true), 1);
    }

    #[test]
    fn test_coins_kept_when_new_pet_is_raised() {
        let mut stats = GameStats::new();
        stats.add_coins(42);
        stats.record_revive();
        assert_eq!(stats.coins, 42);
    }

    #[test]
    fn test_record_feed_increments() {
        let mut stats = GameStats::new();
        stats.record_feed();
        assert_eq!(stats.total_feeds, 1);
        stats.record_feed();
        assert_eq!(stats.total_feeds, 2);
    }

    #[test]
    fn test_record_play_increments() {
        let mut stats = GameStats::new();
        stats.record_play();
        assert_eq!(stats.total_plays, 1);
    }

    #[test]
    fn test_record_sleep_increments() {
        let mut stats = GameStats::new();
        stats.record_sleep();
        assert_eq!(stats.total_sleeps, 1);
    }

    #[test]
    fn test_record_revive_increments() {
        let mut stats = GameStats::new();
        assert_eq!(stats.total_pets_raised, 1);
        stats.record_revive();
        assert_eq!(stats.total_pets_raised, 2);
    }

    #[test]
    fn test_first_care_starts_streak() {
        let mut stats = GameStats::new();
        stats.record_feed();
        assert_eq!(stats.current_streak, 1);
        assert_eq!(stats.longest_streak, 1);
        assert!(stats.last_care_date.is_some());
    }

    #[test]
    fn test_same_day_care_no_streak_change() {
        let mut stats = GameStats::new();
        stats.record_feed();
        assert_eq!(stats.current_streak, 1);
        stats.record_feed();
        assert_eq!(stats.current_streak, 1); // same day, no change
    }

    #[test]
    fn test_streak_resets_on_gap() {
        let mut stats = GameStats::new();
        // Simulate care two days ago
        stats.last_care_date = Some("2020-01-01".to_string());
        stats.current_streak = 5;
        stats.longest_streak = 5;

        stats.record_feed(); // today is not Jan 2, so gap resets
        assert_eq!(stats.current_streak, 1);
        assert_eq!(stats.longest_streak, 5); // longest preserved
    }

    #[test]
    fn test_unlock_achievement() {
        let mut stats = GameStats::new();
        assert!(stats.unlock_achievement("first_hatch"));
        assert!(!stats.unlock_achievement("first_hatch")); // duplicate
        assert_eq!(stats.unlocked.len(), 1);
    }

    #[test]
    fn test_longest_streak_preserved() {
        let mut stats = GameStats::new();
        stats.current_streak = 10;
        stats.longest_streak = 10;
        stats.last_care_date = Some("2020-01-01".to_string());

        // Gap resets current but longest stays
        stats.record_feed();
        assert_eq!(stats.current_streak, 1);
        assert_eq!(stats.longest_streak, 10);
    }
}

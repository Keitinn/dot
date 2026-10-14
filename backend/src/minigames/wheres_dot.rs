//! WHERE'S DOT?: dot hides under one of 3 cups, the cups swap 4-6 times,
//! and the player picks the cup dot is under, over 3 rounds. Each round's
//! start and swaps are shown only when that round comes up; the cup dot ends
//! under is never sent before the pick.

use rand::Rng;
use serde::Serialize;

use super::Settled;
use crate::pet::economy;

pub const CUPS: u8 = 3;
pub const ROUNDS: usize = 3;
pub const MIN_SWAPS: usize = 4;
pub const MAX_SWAPS: usize = 6;

/// Two cup positions (0-2, left to right) that trade places.
pub type Swap = [u8; 2];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Round {
    start: u8,
    swaps: Vec<Swap>,
}

impl Round {
    fn random(rng: &mut impl Rng) -> Self {
        let count = rng.random_range(MIN_SWAPS..=MAX_SWAPS);
        let swaps = (0..count)
            .map(|_| {
                let a = rng.random_range(0..CUPS);
                let b = (a + rng.random_range(1..CUPS)) % CUPS;
                [a.min(b), a.max(b)]
            })
            .collect();
        Self { start: rng.random_range(0..CUPS), swaps }
    }

    /// Where dot is after the swaps.
    fn end(&self) -> u8 {
        self.swaps.iter().fold(self.start, |at, &[a, b]| {
            if at == a {
                b
            } else if at == b {
                a
            } else {
                at
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Pick {
    pub cup: u8,
    /// Where dot was, shown once picked
    pub dot: u8,
    pub correct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// The round being played (1-based); the last one once over
    pub round: u32,
    /// Where dot hides before the swaps; `None` once over
    pub start: Option<u8>,
    /// This round's swaps, in order; empty once over
    pub swaps: Vec<Swap>,
    pub picks: Vec<Pick>,
    pub correct: u32,
}

#[derive(Debug, Clone)]
pub struct WheresDot {
    rounds: Vec<Round>,
    picks: Vec<Pick>,
}

impl WheresDot {
    pub fn new(rng: &mut impl Rng) -> Self {
        Self { rounds: (0..ROUNDS).map(|_| Round::random(rng)).collect(), picks: Vec::new() }
    }

    #[cfg(test)]
    pub(crate) fn with_rounds(rounds: [(u8, &[Swap]); ROUNDS]) -> Self {
        let rounds = rounds
            .iter()
            .map(|&(start, swaps)| Round { start, swaps: swaps.to_vec() })
            .collect();
        Self { rounds, picks: Vec::new() }
    }

    fn is_over(&self) -> bool {
        self.picks.len() >= ROUNDS
    }

    fn correct(&self) -> u32 {
        self.picks.iter().filter(|p| p.correct).count() as u32
    }

    /// A cup that is not 0-2 is refused and changes nothing.
    pub fn pick(&mut self, cup: u8) -> Result<Pick, String> {
        if self.is_over() {
            return Err("game is over".to_string());
        }
        if cup >= CUPS {
            return Err("a cup is 0-2".to_string());
        }
        let dot = self.rounds[self.picks.len()].end();
        let pick = Pick { cup, dot, correct: cup == dot };
        self.picks.push(pick);
        Ok(pick)
    }

    pub fn settled(&self) -> Option<Settled> {
        self.is_over().then(|| Settled {
            reward: economy::wheres_dot_reward(self.correct()),
            moves: self.rounds.iter().map(|r| r.swaps.len() as u32).sum(),
        })
    }

    pub fn view(&self) -> View {
        let current = (!self.is_over()).then(|| &self.rounds[self.picks.len()]);
        View {
            round: (self.picks.len() + 1).min(ROUNDS) as u32,
            start: current.map(|r| r.start),
            swaps: current.map(|r| r.swaps.clone()).unwrap_or_default(),
            picks: self.picks.clone(),
            correct: self.correct(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    // Dot ends under cup 2, then 0, then 1
    fn game() -> WheresDot {
        WheresDot::with_rounds([
            (0, &[[0, 1], [1, 2], [0, 1], [0, 1]]),
            (1, &[[1, 2], [0, 2], [0, 1], [1, 2], [0, 2]]),
            (2, &[[0, 1], [0, 1], [1, 2], [0, 2], [0, 1], [0, 1]]),
        ])
    }

    #[test]
    fn dot_follows_the_swaps() {
        let g = game();
        assert_eq!(g.rounds.iter().map(Round::end).collect::<Vec<_>>(), vec![2, 0, 1]);
    }

    #[test]
    fn a_pick_is_judged_and_the_next_round_shows() {
        let mut g = game();
        assert_eq!(g.pick(2), Ok(Pick { cup: 2, dot: 2, correct: true }));
        let view = g.view();
        assert_eq!(view.round, 2);
        assert_eq!(view.start, Some(1));
        assert_eq!(view.swaps.len(), 5);
        assert_eq!(g.pick(1), Ok(Pick { cup: 1, dot: 0, correct: false }));
        assert!(g.settled().is_none());
    }

    #[test]
    fn rewards_by_right_picks_with_a_bonus_for_all_three() {
        for (picks, coins) in [([0, 1, 0], 0), ([2, 1, 0], 1), ([2, 0, 0], 2), ([2, 0, 1], 4)] {
            let mut g = game();
            for cup in picks {
                g.pick(cup).unwrap();
            }
            assert_eq!(g.settled(), Some(Settled { reward: coins, moves: 15 }), "{picks:?}");
        }
    }

    #[test]
    fn after_three_rounds_the_game_is_over() {
        let mut g = game();
        for cup in [2, 0, 1] {
            g.pick(cup).unwrap();
        }
        assert!(g.pick(0).is_err());
        let view = g.view();
        assert_eq!((view.round, view.start, view.swaps.len(), view.correct), (3, None, 0, 3));
    }

    #[test]
    fn a_bad_cup_is_refused_and_changes_nothing() {
        let mut g = game();
        assert!(g.pick(3).is_err());
        assert!(g.pick(255).is_err());
        assert_eq!(g.view().round, 1);
        assert!(g.view().picks.is_empty());
    }

    #[test]
    fn the_view_never_holds_where_dot_ends_before_the_pick() {
        let g = game();
        let json = serde_json::to_value(g.view()).unwrap();
        let keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.len(), 5, "{json}");
        for key in ["round", "start", "swaps", "picks", "correct"] {
            assert!(keys.contains(&key), "{key} missing in {json}");
        }
        assert_eq!(json["picks"], serde_json::json!([]));
        // Only round 1 is shown
        assert_eq!(json["swaps"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn rounds_swap_4_to_6_times_between_two_cups_and_follow_the_seed() {
        let mut rng = StdRng::seed_from_u64(3);
        for _ in 0..200 {
            let g = WheresDot::new(&mut rng);
            for r in &g.rounds {
                assert!((MIN_SWAPS..=MAX_SWAPS).contains(&r.swaps.len()));
                assert!(r.start < CUPS);
                assert!(r.swaps.iter().all(|&[a, b]| a < b && b < CUPS));
            }
        }
        let a = WheresDot::new(&mut StdRng::seed_from_u64(8)).rounds;
        let b = WheresDot::new(&mut StdRng::seed_from_u64(8)).rounds;
        assert_eq!(a, b);
    }
}

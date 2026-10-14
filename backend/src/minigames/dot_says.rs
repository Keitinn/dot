//! DOT SAYS: repeat the order in which dot's 4 pads light up. Each round
//! adds one light; one miss ends the game. Only the current round's order
//! is shown, so the rest of the sequence stays here.

use rand::Rng;
use serde::Serialize;

use super::Settled;
use crate::pet::economy;

pub const PADS: u8 = 4;
/// Clearing this many rounds pays the most, so the game ends there.
pub const MAX_ROUNDS: u32 = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// The round being played (1-based); the last one played once over
    pub round: u32,
    /// The order to repeat this round; empty once over
    pub show: Vec<u8>,
    pub cleared: u32,
    pub missed: bool,
}

#[derive(Debug, Clone)]
pub struct DotSays {
    sequence: Vec<u8>,
    cleared: u32,
    missed: bool,
}

impl DotSays {
    pub fn new(rng: &mut impl Rng) -> Self {
        Self::with_sequence((0..MAX_ROUNDS).map(|_| rng.random_range(0..PADS)).collect())
    }

    pub(crate) fn with_sequence(sequence: Vec<u8>) -> Self {
        assert_eq!(sequence.len(), MAX_ROUNDS as usize);
        Self { sequence, cleared: 0, missed: false }
    }

    fn is_over(&self) -> bool {
        self.missed || self.cleared >= MAX_ROUNDS
    }

    fn round(&self) -> u32 {
        (self.cleared + 1).min(MAX_ROUNDS)
    }

    /// The pads tapped so far this round. A wrong pad ends the game; the
    /// whole order clears the round. A right but unfinished order, an empty
    /// one or an unknown pad is refused and changes nothing.
    pub fn say(&mut self, order: &[u8]) -> Result<(), String> {
        if self.is_over() {
            return Err("game is over".to_string());
        }
        let expected = &self.sequence[..self.round() as usize];
        if order.is_empty() || order.len() > expected.len() {
            return Err(format!("this round is {} pads", expected.len()));
        }
        if order.iter().any(|&pad| pad >= PADS) {
            return Err("a pad is 0-3".to_string());
        }
        if order != &expected[..order.len()] {
            self.missed = true;
        } else if order.len() == expected.len() {
            self.cleared += 1;
        } else {
            return Err("the round is not done".to_string());
        }
        Ok(())
    }

    pub fn settled(&self) -> Option<Settled> {
        self.is_over().then(|| Settled {
            reward: economy::dot_says_reward(self.cleared),
            moves: self.cleared,
        })
    }

    pub fn view(&self) -> View {
        let round = self.round();
        View {
            round,
            show: if self.is_over() { Vec::new() } else { self.sequence[..round as usize].to_vec() },
            cleared: self.cleared,
            missed: self.missed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    const SEQ: [u8; 8] = [2, 0, 3, 3, 1, 0, 2, 1];

    fn game() -> DotSays {
        DotSays::with_sequence(SEQ.to_vec())
    }

    fn clear(g: &mut DotSays, rounds: u32) {
        for r in 1..=rounds as usize {
            g.say(&SEQ[..r]).unwrap();
        }
    }

    #[test]
    fn each_round_shows_one_more_pad() {
        let mut g = game();
        assert_eq!(g.view().show, vec![2]);
        g.say(&[2]).unwrap();
        assert_eq!(g.view(), View { round: 2, show: vec![2, 0], cleared: 1, missed: false });
        g.say(&[2, 0]).unwrap();
        assert_eq!(g.view().show, vec![2, 0, 3]);
    }

    #[test]
    fn one_miss_ends_the_game() {
        let mut g = game();
        clear(&mut g, 3);
        g.say(&[2, 1]).unwrap();
        let view = g.view();
        assert!(view.missed);
        assert!(view.show.is_empty());
        assert_eq!(g.settled(), Some(Settled { reward: 1, moves: 3 }));
        assert!(g.say(&SEQ[..4]).is_err());
    }

    #[test]
    fn a_miss_on_the_first_pad_of_round_one_pays_nothing() {
        let mut g = game();
        g.say(&[1]).unwrap();
        assert_eq!(g.settled(), Some(Settled { reward: 0, moves: 0 }));
    }

    #[test]
    fn reward_steps_by_rounds_cleared() {
        for (rounds, coins) in [(2, 0), (3, 1), (4, 1), (5, 3), (7, 3)] {
            let mut g = game();
            clear(&mut g, rounds);
            assert!(g.settled().is_none());
            g.say(&[(SEQ[0] + 1) % PADS]).unwrap();
            assert_eq!(g.settled(), Some(Settled { reward: coins, moves: rounds }), "{rounds} rounds");
        }
    }

    #[test]
    fn clearing_all_eight_rounds_pays_5_and_ends() {
        let mut g = game();
        clear(&mut g, MAX_ROUNDS);
        assert_eq!(g.settled(), Some(Settled { reward: 5, moves: 8 }));
        assert!(g.view().show.is_empty());
        assert!(g.say(&SEQ).is_err());
    }

    #[test]
    fn an_unfinished_or_bad_order_is_refused_and_changes_nothing() {
        let mut g = game();
        clear(&mut g, 2);
        for bad in [&[][..], &[2, 0], &[2, 0, 3, 3], &[2, 0, 4], &[9]] {
            assert!(g.say(bad).is_err(), "{bad:?} was accepted");
        }
        assert_eq!(g.view(), View { round: 3, show: vec![2, 0, 3], cleared: 2, missed: false });
    }

    #[test]
    fn the_view_shows_only_this_rounds_order() {
        let mut g = game();
        clear(&mut g, 4);
        let json = serde_json::to_value(g.view()).unwrap();
        assert_eq!(json["show"], serde_json::json!([2, 0, 3, 3, 1]));
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), 4, "{json}");
    }

    #[test]
    fn the_sequence_uses_4_pads_and_follows_the_seed() {
        let a = DotSays::new(&mut StdRng::seed_from_u64(5));
        let b = DotSays::new(&mut StdRng::seed_from_u64(5));
        assert_eq!(a.sequence, b.sequence);
        assert_eq!(a.sequence.len(), MAX_ROUNDS as usize);
        let mut rng = StdRng::seed_from_u64(6);
        for _ in 0..100 {
            assert!(DotSays::new(&mut rng).sequence.iter().all(|&p| p < PADS));
        }
    }
}

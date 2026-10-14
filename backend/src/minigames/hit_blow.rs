//! HIT & BLOW: guess a 3-digit number with no repeated digit in 8 tries.
//! HIT counts digits in the right place, BLOW digits in the wrong place.

use rand::{seq::SliceRandom, Rng};
use serde::Serialize;

use super::Settled;
use crate::pet::economy;

pub const DIGITS: usize = 3;
pub const MAX_GUESSES: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Row {
    pub digits: [u8; DIGITS],
    pub hit: u8,
    pub blow: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    pub guesses: Vec<Row>,
    pub left: u32,
    pub solved: bool,
    /// Shown only once the game is over
    pub answer: Option<[u8; DIGITS]>,
}

#[derive(Debug, Clone)]
pub struct HitBlow {
    answer: [u8; DIGITS],
    guesses: Vec<Row>,
}

impl HitBlow {
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut pool: Vec<u8> = (0..10).collect();
        pool.shuffle(rng);
        Self::with_answer([pool[0], pool[1], pool[2]])
    }

    pub(crate) fn with_answer(answer: [u8; DIGITS]) -> Self {
        Self { answer, guesses: Vec::new() }
    }

    /// A guess that is not 3 distinct digits is refused and costs no try.
    pub fn guess(&mut self, digits: &[u8]) -> Result<Row, String> {
        let digits = valid_guess(digits)?;
        let hit = digits.iter().zip(&self.answer).filter(|(g, a)| g == a).count() as u8;
        let shared = digits.iter().filter(|d| self.answer.contains(d)).count() as u8;
        let row = Row { digits, hit, blow: shared - hit };
        self.guesses.push(row);
        Ok(row)
    }

    fn solved(&self) -> bool {
        self.guesses.last().is_some_and(|r| r.hit as usize == DIGITS)
    }

    pub fn settled(&self) -> Option<Settled> {
        let tries = self.guesses.len() as u32;
        let solved = self.solved();
        (solved || tries >= MAX_GUESSES).then(|| Settled {
            reward: economy::hit_blow_reward(solved.then_some(tries)),
            moves: tries,
        })
    }

    pub fn view(&self) -> View {
        View {
            guesses: self.guesses.clone(),
            left: MAX_GUESSES - self.guesses.len() as u32,
            solved: self.solved(),
            answer: self.settled().map(|_| self.answer),
        }
    }
}

fn valid_guess(digits: &[u8]) -> Result<[u8; DIGITS], String> {
    let digits: [u8; DIGITS] = digits
        .try_into()
        .map_err(|_| format!("a guess is {DIGITS} digits"))?;
    if digits.iter().any(|&d| d > 9) {
        return Err("a digit is 0-9".to_string());
    }
    if digits[0] == digits[1] || digits[0] == digits[2] || digits[1] == digits[2] {
        return Err("digits must differ".to_string());
    }
    Ok(digits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    fn row(digits: [u8; 3], hit: u8, blow: u8) -> Row {
        Row { digits, hit, blow }
    }

    #[test]
    fn counts_hits_and_blows() {
        let mut g = HitBlow::with_answer([1, 2, 3]);
        assert_eq!(g.guess(&[4, 5, 6]), Ok(row([4, 5, 6], 0, 0)));
        assert_eq!(g.guess(&[3, 1, 2]), Ok(row([3, 1, 2], 0, 3)));
        assert_eq!(g.guess(&[1, 3, 9]), Ok(row([1, 3, 9], 1, 1)));
        assert_eq!(g.guess(&[0, 2, 3]), Ok(row([0, 2, 3], 2, 0)));
        assert!(g.settled().is_none());
        assert_eq!(g.guess(&[1, 2, 3]), Ok(row([1, 2, 3], 3, 0)));
        assert_eq!(g.settled(), Some(Settled { reward: 4, moves: 5 }));
    }

    #[test]
    fn solving_on_the_first_try_pays_8() {
        let mut g = HitBlow::with_answer([0, 9, 8]);
        g.guess(&[0, 9, 8]).unwrap();
        assert_eq!(g.settled(), Some(Settled { reward: 8, moves: 1 }));
        assert!(g.view().solved);
    }

    #[test]
    fn eight_misses_end_the_game_with_nothing() {
        let mut g = HitBlow::with_answer([1, 2, 3]);
        for i in 0..MAX_GUESSES {
            assert!(g.settled().is_none(), "over after {i} guesses");
            g.guess(&[4, 5, 6]).unwrap();
        }
        assert_eq!(g.settled(), Some(Settled { reward: 0, moves: 8 }));
        assert_eq!(g.view().answer, Some([1, 2, 3]));
    }

    #[test]
    fn solving_on_the_last_try_pays_1() {
        let mut g = HitBlow::with_answer([1, 2, 3]);
        for _ in 0..MAX_GUESSES - 1 {
            g.guess(&[4, 5, 6]).unwrap();
        }
        g.guess(&[1, 2, 3]).unwrap();
        assert_eq!(g.settled(), Some(Settled { reward: 1, moves: 8 }));
    }

    #[test]
    fn a_bad_guess_is_refused_and_costs_no_try() {
        let mut g = HitBlow::with_answer([1, 2, 3]);
        for bad in [&[1, 1, 2][..], &[1, 2], &[1, 2, 3, 4], &[], &[1, 2, 10], &[3, 4, 3]] {
            assert!(g.guess(bad).is_err(), "{bad:?} was accepted");
        }
        assert_eq!(g.view().left, MAX_GUESSES);
        assert!(g.view().guesses.is_empty());
    }

    #[test]
    fn the_answer_is_hidden_until_the_game_is_over() {
        let mut g = HitBlow::with_answer([1, 2, 3]);
        g.guess(&[4, 5, 6]).unwrap();
        assert_eq!(g.view().answer, None);
        let json = serde_json::to_value(g.view()).unwrap();
        assert_eq!(json["answer"], serde_json::Value::Null);
        g.guess(&[1, 2, 3]).unwrap();
        assert_eq!(g.view().answer, Some([1, 2, 3]));
    }

    #[test]
    fn answers_have_three_distinct_digits_and_follow_the_seed() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..200 {
            let g = HitBlow::new(&mut rng);
            assert!(valid_guess(&g.answer).is_ok(), "{:?}", g.answer);
        }
        let a = HitBlow::new(&mut StdRng::seed_from_u64(42)).answer;
        let b = HitBlow::new(&mut StdRng::seed_from_u64(42)).answer;
        assert_eq!(a, b);
    }
}

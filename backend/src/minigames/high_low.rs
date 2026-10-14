//! HIGH & LOW: call whether the next card is higher or lower. A tie keeps
//! the streak without adding to it; a miss ends the game with nothing.
//! STOP takes the coins for the streak so far.

use rand::{seq::SliceRandom, Rng};
use serde::Serialize;

use super::Settled;
use crate::pet::economy;

/// The streak that pays the most; the game ends there.
pub const MAX_STREAK: u32 = 5;

/// `rank` 1-13 (A=1, K=13); `suit` 0-3 for ♠♥♦♣.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Card {
    pub rank: u8,
    pub suit: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Call {
    High,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Win,
    Draw,
    Lose,
}

/// The last call and the card it drew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Turn {
    pub call: Call,
    pub card: Card,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum End {
    Lost,
    Stopped,
    DeckOut,
    Max,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct View {
    /// The face-up card the next call is against
    pub card: Card,
    pub streak: u32,
    /// The coins STOP would take now
    pub banked: u64,
    /// Cards still in the deck
    pub left: u32,
    pub last: Option<Turn>,
    pub end: Option<End>,
}

#[derive(Debug, Clone)]
pub struct HighLow {
    /// Drawn from the back
    deck: Vec<Card>,
    current: Card,
    streak: u32,
    calls: u32,
    last: Option<Turn>,
    end: Option<End>,
}

impl HighLow {
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut cards: Vec<Card> = (0..4u8)
            .flat_map(|suit| (1..=13u8).map(move |rank| Card { rank, suit }))
            .collect();
        cards.shuffle(rng);
        Self::with_order(cards)
    }

    /// `order[0]` is the first face-up card, then the draws in order.
    pub(crate) fn with_order(mut order: Vec<Card>) -> Self {
        order.reverse();
        let current = order.pop().expect("a deck has cards");
        let mut game = Self { deck: order, current, streak: 0, calls: 0, last: None, end: None };
        if game.deck.is_empty() {
            game.end = Some(End::DeckOut);
        }
        game
    }

    pub fn call(&mut self, call: Call) -> Result<Turn, String> {
        if self.end.is_some() {
            return Err("game is over".to_string());
        }
        let card = self.deck.pop().ok_or_else(|| "no cards left".to_string())?;
        let verdict = match (card.rank.cmp(&self.current.rank), call) {
            (std::cmp::Ordering::Equal, _) => Verdict::Draw,
            (std::cmp::Ordering::Greater, Call::High) | (std::cmp::Ordering::Less, Call::Low) => Verdict::Win,
            _ => Verdict::Lose,
        };
        self.calls += 1;
        self.current = card;
        match verdict {
            Verdict::Win => self.streak += 1,
            Verdict::Lose => self.end = Some(End::Lost),
            Verdict::Draw => {}
        }
        if self.end.is_none() {
            if self.streak >= MAX_STREAK {
                self.end = Some(End::Max);
            } else if self.deck.is_empty() {
                self.end = Some(End::DeckOut);
            }
        }
        let turn = Turn { call, card, verdict };
        self.last = Some(turn);
        Ok(turn)
    }

    pub fn stop(&mut self) -> Result<(), String> {
        if self.end.is_some() {
            return Err("game is over".to_string());
        }
        self.end = Some(End::Stopped);
        Ok(())
    }

    fn banked(&self) -> u64 {
        if self.end == Some(End::Lost) {
            0
        } else {
            economy::high_low_reward(self.streak)
        }
    }

    pub fn settled(&self) -> Option<Settled> {
        self.end.map(|_| Settled { reward: self.banked(), moves: self.calls })
    }

    pub fn view(&self) -> View {
        View {
            card: self.current,
            streak: self.streak,
            banked: self.banked(),
            left: self.deck.len() as u32,
            last: self.last,
            end: self.end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};

    fn cards(ranks: &[u8]) -> Vec<Card> {
        ranks.iter().map(|&rank| Card { rank, suit: 0 }).collect()
    }

    #[test]
    fn right_calls_build_a_streak() {
        let mut g = HighLow::with_order(cards(&[7, 9, 3, 13, 1, 2]));
        assert_eq!(g.call(Call::High).unwrap().verdict, Verdict::Win);
        assert_eq!(g.call(Call::Low).unwrap().verdict, Verdict::Win);
        assert_eq!(g.call(Call::High).unwrap().verdict, Verdict::Win);
        // K then A: A is 1, the lowest
        assert_eq!(g.call(Call::Low).unwrap().verdict, Verdict::Win);
        assert_eq!(g.view().streak, 4);
        assert_eq!(g.view().banked, 4);
        assert!(g.settled().is_none());
    }

    #[test]
    fn a_tie_keeps_the_streak_without_adding() {
        let mut g = HighLow::with_order(cards(&[5, 8, 8, 2]));
        g.call(Call::High).unwrap();
        let tie = g.call(Call::High).unwrap();
        assert_eq!(tie.verdict, Verdict::Draw);
        assert_eq!(g.view().streak, 1);
        assert!(g.settled().is_none());
        g.call(Call::Low).unwrap();
        assert_eq!(g.view().streak, 2);
    }

    #[test]
    fn a_miss_ends_the_game_with_nothing() {
        let mut g = HighLow::with_order(cards(&[5, 8, 9, 2, 10]));
        g.call(Call::High).unwrap();
        g.call(Call::High).unwrap();
        assert_eq!(g.call(Call::High).unwrap().verdict, Verdict::Lose);
        assert_eq!(g.view().end, Some(End::Lost));
        assert_eq!(g.settled(), Some(Settled { reward: 0, moves: 3 }));
        assert!(g.call(Call::Low).is_err());
        assert!(g.stop().is_err());
    }

    #[test]
    fn stop_takes_the_streak_so_far() {
        let mut g = HighLow::with_order(cards(&[5, 8, 9, 2, 10]));
        g.call(Call::High).unwrap();
        g.call(Call::High).unwrap();
        g.stop().unwrap();
        assert_eq!(g.view().end, Some(End::Stopped));
        assert_eq!(g.settled(), Some(Settled { reward: 2, moves: 2 }));
        assert!(g.call(Call::High).is_err());
    }

    #[test]
    fn stop_before_any_call_pays_nothing() {
        let mut g = HighLow::with_order(cards(&[5, 8]));
        g.stop().unwrap();
        assert_eq!(g.settled(), Some(Settled { reward: 0, moves: 0 }));
    }

    #[test]
    fn the_game_ends_at_the_top_streak() {
        let mut g = HighLow::with_order(cards(&[1, 2, 3, 4, 5, 6, 7]));
        for _ in 0..MAX_STREAK {
            g.call(Call::High).unwrap();
        }
        assert_eq!(g.view().end, Some(End::Max));
        assert_eq!(g.settled(), Some(Settled { reward: 5, moves: 5 }));
    }

    #[test]
    fn an_empty_deck_ends_the_game_and_keeps_the_streak() {
        let mut g = HighLow::with_order(cards(&[2, 3, 3]));
        g.call(Call::High).unwrap();
        assert!(g.settled().is_none());
        g.call(Call::Low).unwrap(); // tie on the last card
        assert_eq!(g.view().end, Some(End::DeckOut));
        assert_eq!(g.view().left, 0);
        assert_eq!(g.settled(), Some(Settled { reward: 1, moves: 2 }));
        assert!(g.call(Call::High).is_err());
    }

    #[test]
    fn a_full_deck_of_ties_runs_out() {
        let mut g = HighLow::with_order(cards(&[4; 52]));
        for _ in 0..51 {
            assert_eq!(g.call(Call::High).unwrap().verdict, Verdict::Draw);
        }
        assert_eq!(g.view().end, Some(End::DeckOut));
        assert_eq!(g.settled(), Some(Settled { reward: 0, moves: 51 }));
    }

    #[test]
    fn the_view_shows_no_card_still_in_the_deck() {
        let g = HighLow::with_order(cards(&[5, 12, 1]));
        let json = serde_json::to_value(g.view()).unwrap();
        let keys: Vec<&str> = json.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.len(), 6, "{json}");
        for key in ["card", "streak", "banked", "left", "last", "end"] {
            assert!(keys.contains(&key), "{key} missing in {json}");
        }
        assert_eq!(json["card"], serde_json::json!({ "rank": 5, "suit": 0 }));
        assert_eq!(json["left"], 2);
    }

    #[test]
    fn a_new_deck_is_52_distinct_cards_dealt_by_the_seed() {
        let g = HighLow::new(&mut StdRng::seed_from_u64(9));
        let mut all: Vec<Card> = g.deck.clone();
        all.push(g.current);
        all.sort_by_key(|c| (c.suit, c.rank));
        all.dedup();
        assert_eq!(all.len(), 52);
        assert!(all.iter().all(|c| (1..=13).contains(&c.rank) && c.suit < 4));

        let again = HighLow::new(&mut StdRng::seed_from_u64(9));
        assert_eq!((g.current, g.deck), (again.current, again.deck));
    }
}

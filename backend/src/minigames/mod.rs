//! Mini games whose answer and dice stay in Rust. The UI sends one move at a
//! time through `game_step` and sees only what that move reveals; the coins
//! of a finished game come from this state, never from the UI's report.

pub mod high_low;
pub mod hit_blow;

use rand::Rng;
use serde::{Deserialize, Serialize};

use high_low::{Call, HighLow};
use hit_blow::HitBlow;

/// One move sent by the UI.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepInput {
    /// Shows the game as it is, without moving
    Look,
    /// HIT & BLOW: 3 distinct digits
    Guess { digits: Vec<u8> },
    /// HIGH & LOW: the next card is higher
    High,
    /// HIGH & LOW: the next card is lower
    Low,
    /// HIGH & LOW: take the coins for the streak so far
    Stop,
}

/// A finished game: its coins before the daily cap, and the count
/// `min_play_time` measures it by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    pub reward: u64,
    pub moves: u32,
}

/// What the UI gets back from a move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepResult {
    pub finished: bool,
    /// Set once finished: what `min_play_time` counts for this game
    pub moves: Option<u32>,
    /// Set once finished: the coins before the daily cap
    pub reward: Option<u64>,
    pub view: View,
}

/// The part of a game the player may see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "game", rename_all = "snake_case")]
pub enum View {
    HitBlow(hit_blow::View),
    HighLow(high_low::View),
}

/// A game in progress, with its secret.
#[derive(Debug, Clone)]
pub enum GameState {
    HitBlow(HitBlow),
    HighLow(HighLow),
}

impl GameState {
    /// `None` for a game the UI plays on its own (janken, memory, blackjack).
    pub fn new(game: &str, rng: &mut impl Rng) -> Option<Self> {
        match game {
            "hit_blow" => Some(Self::HitBlow(HitBlow::new(rng))),
            "high_low" => Some(Self::HighLow(HighLow::new(rng))),
            _ => None,
        }
    }

    pub fn settled(&self) -> Option<Settled> {
        match self {
            Self::HitBlow(g) => g.settled(),
            Self::HighLow(g) => g.settled(),
        }
    }

    /// A move that does not fit the game is refused and changes nothing.
    pub fn step(&mut self, input: &StepInput) -> Result<StepResult, String> {
        if self.settled().is_some() && *input != StepInput::Look {
            return Err("game is over".to_string());
        }
        match (&mut *self, input) {
            (_, StepInput::Look) => {}
            (Self::HitBlow(g), StepInput::Guess { digits }) => {
                g.guess(digits)?;
            }
            (Self::HighLow(g), StepInput::High) => {
                g.call(Call::High)?;
            }
            (Self::HighLow(g), StepInput::Low) => {
                g.call(Call::Low)?;
            }
            (Self::HighLow(g), StepInput::Stop) => g.stop()?,
            _ => return Err("not a move in this game".to_string()),
        }
        Ok(self.result())
    }

    fn view(&self) -> View {
        match self {
            Self::HitBlow(g) => View::HitBlow(g.view()),
            Self::HighLow(g) => View::HighLow(g.view()),
        }
    }

    fn result(&self) -> StepResult {
        let settled = self.settled();
        StepResult {
            finished: settled.is_some(),
            moves: settled.map(|s| s.moves),
            reward: settled.map(|s| s.reward),
            view: self.view(),
        }
    }
}

use serde::Serialize;

/// A purchasable snack. Effects are added to the pet's stats (capped at 100).
/// Snacks are independent of the FEED button and its cooldown.
#[derive(Debug, Clone, Serialize)]
pub struct Snack {
    pub id: &'static str,
    pub name: &'static str,
    pub price: u64,
    pub hunger: u8,
    pub happiness: u8,
    pub energy: u8,
    pub health: u8,
}

pub const SNACKS: [Snack; 5] = [
    Snack { id: "candy",    name: "CANDY",    price: 3,  hunger: 5,  happiness: 8,  energy: 0,  health: 0 },
    Snack { id: "cookie",   name: "COOKIE",   price: 8,  hunger: 15, happiness: 10, energy: 0,  health: 0 },
    Snack { id: "cake",     name: "CAKE",     price: 20, hunger: 20, happiness: 25, energy: 0,  health: 0 },
    Snack { id: "coffee",   name: "COFFEE",   price: 10, hunger: 0,  happiness: 0,  energy: 25, health: 0 },
    Snack { id: "medicine", name: "MEDICINE", price: 30, hunger: 0,  happiness: 0,  energy: 0,  health: 30 },
];

pub fn find_snack(id: &str) -> Option<&'static Snack> {
    SNACKS.iter().find(|s| s.id == id)
}

/// Coins earned for a finished mini game. Games have no stake, so playing
/// never costs coins: a pet with 0 coins can still earn by playing or waiting.
pub fn game_reward(game: &str, outcome: &str, moves: Option<u32>) -> Result<u64, String> {
    match (game, outcome) {
        ("janken", "win") => Ok(2),
        ("janken", "draw") => Ok(1),
        ("janken", "lose") => Ok(0),
        ("blackjack", "blackjack") => Ok(8),
        ("blackjack", "win") => Ok(5),
        ("blackjack", "push") => Ok(1),
        ("blackjack", "lose") => Ok(0),
        ("memory", "clear") => {
            // 8 cards -> 4 moves is perfect (10 coins); 1 coin less per extra move, min 1.
            let m = moves.ok_or_else(|| "moves required for memory".to_string())?;
            Ok(10u32.saturating_sub(m.saturating_sub(4)).max(1) as u64)
        }
        _ => Err(format!("unknown game result: {game}/{outcome}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_snack() {
        assert_eq!(find_snack("cake").unwrap().price, 20);
        assert!(find_snack("pizza").is_none());
    }

    #[test]
    fn test_snack_ids_unique_and_priced() {
        for (i, a) in SNACKS.iter().enumerate() {
            assert!(a.price > 0);
            for b in SNACKS.iter().skip(i + 1) {
                assert_ne!(a.id, b.id);
            }
        }
    }

    #[test]
    fn test_janken_rewards() {
        assert_eq!(game_reward("janken", "win", None), Ok(2));
        assert_eq!(game_reward("janken", "draw", None), Ok(1));
        assert_eq!(game_reward("janken", "lose", None), Ok(0));
    }

    #[test]
    fn test_blackjack_rewards() {
        assert_eq!(game_reward("blackjack", "blackjack", None), Ok(8));
        assert_eq!(game_reward("blackjack", "win", None), Ok(5));
        assert_eq!(game_reward("blackjack", "push", None), Ok(1));
        assert_eq!(game_reward("blackjack", "lose", None), Ok(0));
    }

    #[test]
    fn test_memory_reward_scales_with_moves() {
        assert_eq!(game_reward("memory", "clear", Some(4)), Ok(10));
        assert_eq!(game_reward("memory", "clear", Some(7)), Ok(7));
        assert_eq!(game_reward("memory", "clear", Some(13)), Ok(1));
        assert_eq!(game_reward("memory", "clear", Some(99)), Ok(1)); // floor of 1
        assert_eq!(game_reward("memory", "clear", Some(0)), Ok(10)); // never above 10
        assert!(game_reward("memory", "clear", None).is_err());
    }

    #[test]
    fn test_unknown_result_rejected() {
        assert!(game_reward("poker", "win", None).is_err());
        assert!(game_reward("janken", "cheat", None).is_err());
    }
}

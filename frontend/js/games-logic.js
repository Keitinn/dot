// Pure game rules (no DOM) so they can be unit-tested with node.

// ---------- Janken (rock paper scissors) ----------
export const JANKEN_HANDS = ['rock', 'paper', 'scissors'];
const JANKEN_BEATS = { rock: 'scissors', scissors: 'paper', paper: 'rock' };

/** Returns 'win' | 'lose' | 'draw' from the player's point of view. */
export function judgeJanken(player, cpu) {
    if (!JANKEN_HANDS.includes(player) || !JANKEN_HANDS.includes(cpu)) {
        throw new Error('invalid hand');
    }
    if (player === cpu) return 'draw';
    return JANKEN_BEATS[player] === cpu ? 'win' : 'lose';
}

export function randomHand(rng = Math.random) {
    return JANKEN_HANDS[Math.floor(rng() * JANKEN_HANDS.length)];
}

// ---------- Shared ----------
export const SUITS = ['♠', '♥', '♦', '♣'];
export const RANKS = ['A', '2', '3', '4', '5', '6', '7', '8', '9', '10', 'J', 'Q', 'K'];

export function shuffle(items, rng = Math.random) {
    const a = items.slice();
    for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(rng() * (i + 1));
        [a[i], a[j]] = [a[j], a[i]];
    }
    return a;
}

// ---------- Memory (神経衰弱) : 4 suits x 2 = 8 cards ----------
export function createMemoryDeck(rng = Math.random) {
    return shuffle([...SUITS, ...SUITS], rng).map((suit, id) => ({
        id,
        suit,
        faceUp: false,
        matched: false,
    }));
}

/**
 * Apply a flip to the board. Returns { ok, pending, matched, done }.
 * `pending` holds ids of two face-up non-matching cards that the UI must flip back.
 */
export function flipMemoryCard(cards, id) {
    const card = cards.find((c) => c.id === id);
    if (!card || card.faceUp || card.matched) return { ok: false };

    const open = cards.filter((c) => c.faceUp && !c.matched);
    if (open.length >= 2) return { ok: false };

    card.faceUp = true;
    const nowOpen = cards.filter((c) => c.faceUp && !c.matched);
    if (nowOpen.length < 2) return { ok: true, pending: null, matched: false, done: false };

    const [a, b] = nowOpen;
    if (a.suit === b.suit) {
        a.matched = true;
        b.matched = true;
        return { ok: true, pending: null, matched: true, done: cards.every((c) => c.matched) };
    }
    return { ok: true, pending: [a.id, b.id], matched: false, done: false };
}

// ---------- Blackjack ----------
export function createBlackjackDeck(rng = Math.random) {
    const deck = [];
    for (const suit of SUITS) for (const rank of RANKS) deck.push({ suit, rank });
    return shuffle(deck, rng);
}

export function handValue(hand) {
    let total = 0;
    let aces = 0;
    for (const { rank } of hand) {
        if (rank === 'A') {
            aces++;
            total += 11;
        } else if (rank === 'J' || rank === 'Q' || rank === 'K') {
            total += 10;
        } else {
            total += Number(rank);
        }
    }
    while (total > 21 && aces > 0) {
        total -= 10;
        aces--;
    }
    return total;
}

export function isBlackjack(hand) {
    return hand.length === 2 && handValue(hand) === 21;
}

/** Dealer draws until 17 or more (stands on soft 17). Mutates `dealer`, draws from `deck`. */
export function dealerPlay(dealer, deck) {
    while (handValue(dealer) < 17) dealer.push(deck.pop());
    return dealer;
}

/** Returns 'win' | 'lose' | 'push' | 'blackjack' from the player's point of view. */
export function judgeBlackjack(player, dealer) {
    const p = handValue(player);
    const d = handValue(dealer);
    if (p > 21) return 'lose';
    const pBJ = isBlackjack(player);
    const dBJ = isBlackjack(dealer);
    if (pBJ && dBJ) return 'push';
    if (pBJ) return 'blackjack';
    if (dBJ) return 'lose';
    if (d > 21) return 'win';
    if (p > d) return 'win';
    if (p < d) return 'lose';
    return 'push';
}

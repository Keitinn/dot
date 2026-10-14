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

/**
 * Shortest time between start_game and the report that the backend accepts
 * (mirrors min_play_time in backend/src/game_session.rs). 0 for unknown games.
 */
export function minPlayMs(game, moves = 0) {
    switch (game) {
        case 'janken': return 1000;
        case 'blackjack': return 3000;
        case 'memory': return Math.max(4000, 500 * (moves || 0));
        // Games checked in Rust: `moves` is the count game_step returned when finished
        case 'hit_blow': return Math.max(3000, 1000 * (moves || 0));
        case 'high_low': return Math.max(2000, 500 * (moves || 0));
        case 'dot_says': {
            const r = moves || 0;
            return Math.max(2000, 600 * ((r * (r + 1)) / 2));
        }
        default: return 0;
    }
}

/** rewardText for the games checked in Rust, where a refused report is shown, not hidden. */
export function settleText(r) {
    return r ? rewardText(r) : ' TRY AGAIN';
}

// ---------- HIT & BLOW (the answer stays in Rust) ----------
export const HIT_BLOW_DIGITS = 3;

/** 3 distinct digits 0-9: what game_step accepts as a guess. */
export function canGuess(digits) {
    return (
        digits.length === HIT_BLOW_DIGITS &&
        digits.every((d) => Number.isInteger(d) && d >= 0 && d <= 9) &&
        new Set(digits).size === digits.length
    );
}

/** The typed digits after tapping `d`: a repeat or a 4th digit is ignored. */
export function typeDigit(typed, d) {
    if (typed.length >= HIT_BLOW_DIGITS || typed.includes(d)) return typed;
    return [...typed, d];
}

export function hitBlowSlots(typed) {
    const slots = [];
    for (let i = 0; i < HIT_BLOW_DIGITS; i++) slots.push(i < typed.length ? String(typed[i]) : '_');
    return slots.join(' ');
}

export function hitBlowRowText(row) {
    return `${row.digits.join('')} ${row.hit}H ${row.blow}B`;
}

// ---------- HIGH & LOW (the deck stays in Rust) ----------
const VERDICT_TEXT = { win: 'RIGHT', draw: 'TIE', lose: 'MISS' };
const HIGH_LOW_END_TEXT = { lost: 'MISS', stopped: 'STOPPED', deck_out: 'NO CARDS LEFT', max: 'TOP STREAK!' };

/** A card from Rust ({ rank: 1-13, suit: 0-3 }) drawn like a blackjack card. */
export function cardText(card) {
    return `${RANKS[card.rank - 1]}${SUITS[card.suit]}`;
}

export function turnText(turn) {
    if (!turn) return '';
    return `${turn.call.toUpperCase()} ${cardText(turn.card)} ${VERDICT_TEXT[turn.verdict]}`;
}

export function highLowEndText(end) {
    return HIGH_LOW_END_TEXT[end] || '';
}

// ---------- DOT SAYS (the order stays in Rust, one round at a time) ----------
/** One light every 800 ms: slow, well above the 600 ms floor. */
export const DOT_SAYS_STEP_MS = 800;
const DOT_SAYS_LIT_MS = 500;

/** When each pad of `show` turns on and off, in ms from the start. */
export function lightSchedule(show, stepMs = DOT_SAYS_STEP_MS) {
    return show.map((pad, i) => ({ pad, onAt: i * stepMs, offAt: i * stepMs + DOT_SAYS_LIT_MS }));
}

/** The order as text for reduced motion: pads 0-3 read as 1-4. */
export function orderText(show) {
    return show.map((pad) => pad + 1).join(' ');
}

/** How long the text order stays up with reduced motion. */
export function revealMs(length) {
    return Math.max(1500, 800 * length);
}

/** 'more' while the taps so far match, 'done' when the round is complete, 'miss' otherwise. */
export function judgeTaps(show, taps) {
    for (let i = 0; i < taps.length; i++) if (taps[i] !== show[i]) return 'miss';
    return taps.length === show.length ? 'done' : 'more';
}

export function dotSaysEndText(view) {
    return view.missed ? `MISS - ${view.cleared} ROUNDS` : `ALL ${view.cleared} ROUNDS!`;
}

/** Suffix for a result line: coins earned, and DAILY LIMIT when the cap cut the reward. */
export function rewardText(r) {
    if (!r) return '';
    const coins = r.earned > 0 ? ` +$${r.earned}` : '';
    return coins + (r.capped ? ' DAILY LIMIT' : '');
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

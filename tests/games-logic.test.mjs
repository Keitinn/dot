import test from 'node:test';
import assert from 'node:assert/strict';
import {
    judgeJanken,
    JANKEN_HANDS,
    createMemoryDeck,
    flipMemoryCard,
    handValue,
    isBlackjack,
    dealerPlay,
    judgeBlackjack,
    createBlackjackDeck,
    SUITS,
    minPlayMs,
    rewardText,
    settleText,
    canGuess,
    typeDigit,
    hitBlowRowText,
    hitBlowSlots,
    cardText,
    turnText,
    highLowEndText,
    DOT_SAYS_STEP_MS,
    lightSchedule,
    orderText,
    revealMs,
    judgeTaps,
    dotSaysEndText,
    SWAP_MS,
    applySwap,
    swapText,
    pickText,
    wheresDotEndText,
} from '../frontend/js/games-logic.js';

const c = (rank, suit = '♠') => ({ rank, suit });

test('janken: all 9 combinations', () => {
    const expected = {
        'rock,rock': 'draw', 'rock,paper': 'lose', 'rock,scissors': 'win',
        'paper,rock': 'win', 'paper,paper': 'draw', 'paper,scissors': 'lose',
        'scissors,rock': 'lose', 'scissors,paper': 'win', 'scissors,scissors': 'draw',
    };
    for (const p of JANKEN_HANDS) for (const q of JANKEN_HANDS) {
        assert.equal(judgeJanken(p, q), expected[`${p},${q}`], `${p} vs ${q}`);
    }
    assert.throws(() => judgeJanken('lizard', 'rock'));
});

test('memory: deck has 8 cards, 2 of each of the 4 suits', () => {
    const deck = createMemoryDeck();
    assert.equal(deck.length, 8);
    for (const s of SUITS) assert.equal(deck.filter((x) => x.suit === s).length, 2);
    assert.deepEqual(deck.map((x) => x.id), [0, 1, 2, 3, 4, 5, 6, 7]);
});

test('memory: match, mismatch, lock and completion', () => {
    // deterministic board: ♠♠ ♥♥ ♦♦ ♣♣
    const cards = [...SUITS, ...SUITS].sort().map((suit, id) => ({ id, suit, faceUp: false, matched: false }));
    const idOf = (suit, n) => cards.filter((x) => x.suit === suit)[n].id;

    // first flip: nothing pending
    assert.deepEqual(flipMemoryCard(cards, idOf('♠', 0)), { ok: true, pending: null, matched: false, done: false });
    // same card again is rejected
    assert.equal(flipMemoryCard(cards, idOf('♠', 0)).ok, false);
    // mismatch -> pending pair
    const mm = flipMemoryCard(cards, idOf('♥', 0));
    assert.equal(mm.ok, true);
    assert.equal(mm.matched, false);
    assert.equal(mm.pending.length, 2);
    // third flip is locked while two are face up
    assert.equal(flipMemoryCard(cards, idOf('♦', 0)).ok, false);
    // UI flips them back
    for (const x of cards) if (mm.pending.includes(x.id)) x.faceUp = false;

    // match the remaining suits one by one
    let last;
    for (const s of SUITS) {
        flipMemoryCard(cards, idOf(s, 0));
        last = flipMemoryCard(cards, idOf(s, 1));
        assert.equal(last.matched, true);
    }
    assert.equal(last.done, true);
    // matched cards can't be flipped
    assert.equal(flipMemoryCard(cards, 0).ok, false);
});

test('blackjack: hand values and aces', () => {
    assert.equal(handValue([c('A'), c('K')]), 21);
    assert.equal(handValue([c('A'), c('A'), c('9')]), 21);
    assert.equal(handValue([c('A'), c('A'), c('A')]), 13);
    assert.equal(handValue([c('K'), c('Q'), c('5')]), 25);
    assert.equal(handValue([c('A'), c('6'), c('K')]), 17);
    assert.equal(handValue([c('10'), c('J')]), 20);
});

test('blackjack: natural only counts for two cards', () => {
    assert.equal(isBlackjack([c('A'), c('K')]), true);
    assert.equal(isBlackjack([c('7'), c('7'), c('7')]), false);
});

test('blackjack: judging', () => {
    assert.equal(judgeBlackjack([c('K'), c('Q'), c('5')], [c('2'), c('3')]), 'lose'); // player bust
    assert.equal(judgeBlackjack([c('K'), c('8')], [c('K'), c('Q'), c('5')]), 'win'); // dealer bust
    assert.equal(judgeBlackjack([c('K'), c('9')], [c('K'), c('8')]), 'win');
    assert.equal(judgeBlackjack([c('K'), c('7')], [c('K'), c('8')]), 'lose');
    assert.equal(judgeBlackjack([c('K'), c('8')], [c('9'), c('9')]), 'push');
    assert.equal(judgeBlackjack([c('A'), c('K')], [c('K'), c('8')]), 'blackjack');
    assert.equal(judgeBlackjack([c('A'), c('K')], [c('A'), c('Q')]), 'push');
    assert.equal(judgeBlackjack([c('7'), c('7'), c('7')], [c('A'), c('K')]), 'lose'); // 21 vs natural
});

test('blackjack: dealer draws to 17 and stands', () => {
    const deck = [c('5'), c('4'), c('3')]; // pop order: 3, 4, 5
    const d = dealerPlay([c('5'), c('2')], deck); // 7 -> 10 -> 14 -> 19
    assert.equal(handValue(d), 19);
    assert.equal(d.length, 5);
    const stand = dealerPlay([c('A'), c('6')], [c('9')]); // soft 17 stands
    assert.equal(stand.length, 2);
});

test('blackjack: deck is 52 unique cards', () => {
    const deck = createBlackjackDeck();
    assert.equal(deck.length, 52);
    assert.equal(new Set(deck.map((x) => x.rank + x.suit)).size, 52);
});

test('reward: minimum play time matches the backend (game_session.rs)', () => {
    assert.equal(minPlayMs('janken'), 1000);
    assert.equal(minPlayMs('blackjack'), 3000);
    assert.equal(minPlayMs('memory', 4), 4000); // 4 s floor
    assert.equal(minPlayMs('memory', 9), 4500); // 0.5 s per move
    assert.equal(minPlayMs('poker'), 0);
});

// The same cases as `minimum_play_times` in backend/src/game_session.rs.
const RUST_MIN_PLAY_MS = [
    ['hit_blow', 1, 3000],
    ['hit_blow', 3, 3000],
    ['hit_blow', 8, 8000],
    ['high_low', 0, 2000],
    ['high_low', 4, 2000],
    ['high_low', 9, 4500],
    ['dot_says', 0, 2000],
    ['dot_says', 1, 2000],
    ['dot_says', 3, 3600],
    ['dot_says', 8, 21600],
    ['wheres_dot', 12, 6000],
    ['wheres_dot', 15, 7500],
    ['wheres_dot', 18, 9000],
];

test('reward: minimum play time of the Rust-checked games matches game_session.rs', () => {
    for (const [game, moves, ms] of RUST_MIN_PLAY_MS) {
        assert.equal(minPlayMs(game, moves), ms, `${game} ${moves}`);
    }
});

test('reward: a failed report says TRY AGAIN instead of nothing', () => {
    assert.equal(settleText(null), ' TRY AGAIN');
    assert.equal(settleText({ earned: 3, capped: false }), ' +$3');
    assert.equal(settleText({ earned: 0, capped: true }), ' DAILY LIMIT');
});

test('hit & blow: a guess is 3 distinct digits', () => {
    assert.equal(canGuess([1, 2, 3]), true);
    assert.equal(canGuess([0, 9, 8]), true);
    assert.equal(canGuess([1, 2]), false);
    assert.equal(canGuess([1, 1, 2]), false);
    assert.equal(canGuess([1, 2, 3, 4]), false);
    assert.equal(canGuess([1, 2, 10]), false);
});

test('hit & blow: typing digits skips repeats and stops at 3', () => {
    assert.deepEqual(typeDigit([], 4), [4]);
    assert.deepEqual(typeDigit([4], 4), [4]);
    assert.deepEqual(typeDigit([4, 0], 7), [4, 0, 7]);
    assert.deepEqual(typeDigit([4, 0, 7], 1), [4, 0, 7]);
});

test('hit & blow: rows and slots read as text', () => {
    assert.equal(hitBlowRowText({ digits: [0, 1, 2], hit: 1, blow: 2 }), '012 1H 2B');
    assert.equal(hitBlowSlots([]), '_ _ _');
    assert.equal(hitBlowSlots([5, 0]), '5 0 _');
});

test('high & low: cards from Rust use the blackjack faces (A=1, K=13)', () => {
    assert.equal(cardText({ rank: 1, suit: 0 }), 'A♠');
    assert.equal(cardText({ rank: 10, suit: 1 }), '10♥');
    assert.equal(cardText({ rank: 13, suit: 3 }), 'K♣');
});

test('high & low: the last call and the end read as text', () => {
    assert.equal(turnText({ call: 'high', card: { rank: 12, suit: 2 }, verdict: 'win' }), 'HIGH Q♦ RIGHT');
    assert.equal(turnText({ call: 'low', card: { rank: 5, suit: 0 }, verdict: 'draw' }), 'LOW 5♠ TIE');
    assert.equal(turnText({ call: 'low', card: { rank: 9, suit: 0 }, verdict: 'lose' }), 'LOW 9♠ MISS');
    assert.equal(turnText(null), '');
    assert.equal(highLowEndText('lost'), 'MISS');
    assert.equal(highLowEndText('stopped'), 'STOPPED');
    assert.equal(highLowEndText('deck_out'), 'NO CARDS LEFT');
    assert.equal(highLowEndText('max'), 'TOP STREAK!');
});

test('dot says: lights come one at a time, at least 600 ms apart, never overlapping', () => {
    const plan = lightSchedule([2, 0, 2]);
    assert.deepEqual(plan.map((l) => l.pad), [2, 0, 2]);
    for (let i = 1; i < plan.length; i++) {
        assert.ok(plan[i].onAt - plan[i - 1].onAt >= 600, 'interval');
        assert.ok(plan[i].onAt > plan[i - 1].offAt, 'a gap, so the same pad twice reads as two');
    }
    assert.ok(DOT_SAYS_STEP_MS >= 600);
    assert.deepEqual(lightSchedule([]), []);
});

test('dot says: the order as text for reduced motion, 1-based', () => {
    assert.equal(orderText([0, 3, 1]), '1 4 2');
    assert.ok(revealMs(1) >= 1500);
    assert.ok(revealMs(8) > revealMs(3));
});

test('dot says: a tap is more, done or a miss', () => {
    assert.equal(judgeTaps([1, 3, 0], [1]), 'more');
    assert.equal(judgeTaps([1, 3, 0], [1, 3, 0]), 'done');
    assert.equal(judgeTaps([1, 3, 0], [1, 2]), 'miss');
});

test('dot says: the end reads as text', () => {
    assert.equal(dotSaysEndText({ cleared: 4, missed: true }), 'MISS - 4 ROUNDS');
    assert.equal(dotSaysEndText({ cleared: 8, missed: false }), 'ALL 8 ROUNDS!');
});

test("where's dot: a swap trades the cups at two positions", () => {
    // slots[cup] = position; cups start at their own position
    assert.deepEqual(applySwap([0, 1, 2], [0, 2]), [2, 1, 0]);
    assert.deepEqual(applySwap([2, 1, 0], [1, 2]), [1, 2, 0]);
    const start = [0, 1, 2];
    applySwap(start, [0, 1]);
    assert.deepEqual(start, [0, 1, 2], 'not mutated');
});

test("where's dot: following the swaps finds the cup dot ends under", () => {
    // dot starts under the cup at position 0
    let slots = [0, 1, 2];
    for (const s of [[0, 1], [1, 2], [0, 1], [0, 1]]) slots = applySwap(slots, s);
    assert.equal(slots[0], 2);
});

test("where's dot: swaps are slow, and read as text for reduced motion", () => {
    assert.ok(SWAP_MS >= 500);
    assert.equal(swapText([0, 2], 0, 5), 'SWAP 1/5: 1 <> 3');
    assert.equal(pickText({ cup: 1, dot: 1, correct: true }), 'FOUND DOT!');
    assert.equal(pickText({ cup: 0, dot: 2, correct: false }), 'NOT THERE');
    assert.equal(wheresDotEndText({ correct: 2 }), '2/3 RIGHT');
    assert.equal(wheresDotEndText({ correct: 3 }), '3/3 RIGHT!');
});

test('reward: text shows coins and the daily limit', () => {
    assert.equal(rewardText({ earned: 5, capped: false }), ' +$5');
    assert.equal(rewardText({ earned: 0, capped: false }), '');
    assert.equal(rewardText({ earned: 2, capped: true }), ' +$2 DAILY LIMIT');
    assert.equal(rewardText({ earned: 0, capped: true }), ' DAILY LIMIT');
    assert.equal(rewardText(null), '');
});

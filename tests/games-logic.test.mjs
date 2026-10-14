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

test('reward: text shows coins and the daily limit', () => {
    assert.equal(rewardText({ earned: 5, capped: false }), ' +$5');
    assert.equal(rewardText({ earned: 0, capped: false }), '');
    assert.equal(rewardText({ earned: 2, capped: true }), ' +$2 DAILY LIMIT');
    assert.equal(rewardText({ earned: 0, capped: true }), ' DAILY LIMIT');
    assert.equal(rewardText(null), '');
});

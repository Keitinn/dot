import test from 'node:test';
import assert from 'node:assert/strict';
import {
    BUBBLE_TTL_MS,
    LINE_MAX_CHARS,
    OPEN_MIN_GAP_MS,
    MOODS,
    splitLine,
    bubbleContent,
    shouldAutoDismiss,
    bubbleDeadline,
    eyeGrid,
    faceMood,
    shouldBlink,
    statusText,
    shouldCallOpen,
    parseHour,
} from '../frontend/js/companion-logic.js';

const adult = (over = {}) => ({ stage: 'Adult', is_alive: true, activity: 'Idle', status: 'content', ...over });
const opened = (over = {}) => ({ enabled: true, mood: 'calm', line: null, onboarding: 'none', ...over });

// Line splitting

test('a short line stays on one row', () => {
    assert.deepEqual(splitLine('I\'M HERE.'), ['I\'M HERE.']);
    assert.deepEqual(splitLine('STAY WITH YOU?'), ['STAY WITH YOU?']);
});

test('a longer line is split at the space that balances the two rows', () => {
    assert.deepEqual(splitLine('BRIGHTER WITH YOU HERE.'), ['BRIGHTER WITH', 'YOU HERE.']);
    assert.deepEqual(splitLine('GOOD TO HAVE YOU HERE.'), ['GOOD TO HAVE', 'YOU HERE.']);
});

test('never more than 2 rows or 24 characters', () => {
    const rows = splitLine('ONE TWO THREE FOUR FIVE SIX SEVEN EIGHT');
    assert.ok(rows.length <= 2);
    assert.ok(rows.join(' ').length <= LINE_MAX_CHARS);
});

test('a line without spaces is cut in the middle', () => {
    assert.deepEqual(splitLine('ABCDEFGHIJKLMNOPQRST'), ['ABCDEFGHIJ', 'KLMNOPQRST']);
});

test('blank or missing text gives no rows', () => {
    assert.deepEqual(splitLine(''), []);
    assert.deepEqual(splitLine('   '), []);
    assert.deepEqual(splitLine(null), []);
});

// What the bubble shows

test('enabled=false shows nothing at all', () => {
    const r = opened({ enabled: false, line: { id: 'x', text: 'HI.' }, onboarding: 'prompt' });
    assert.equal(bubbleContent(r), null);
    assert.equal(bubbleContent(null), null);
});

test('a line is shown as a bubble', () => {
    const r = opened({ line: { id: 'greet_any_1', text: 'GOOD TO SEE YOU.' } });
    assert.deepEqual(bubbleContent(r), { kind: 'line', rows: splitLine('GOOD TO SEE YOU.') });
});

test('silence shows no bubble', () => {
    assert.equal(bubbleContent(opened()), null);
    assert.equal(bubbleContent(opened({ line: { id: 'x', text: '' } })), null);
});

test('the onboarding prompt asks STAY WITH YOU? with YES and LATER', () => {
    const r = opened({ onboarding: 'prompt' });
    assert.deepEqual(bubbleContent(r), { kind: 'prompt', rows: ['STAY WITH YOU?'], answers: ['yes', 'later'] });
});

test('the prompt wins over a stray line', () => {
    const r = opened({ onboarding: 'prompt', line: { id: 'x', text: 'HI.' } });
    assert.equal(bubbleContent(r).kind, 'prompt');
});

// TTL and hover

test('the bubble fades BUBBLE_TTL after it is shown', () => {
    assert.equal(BUBBLE_TTL_MS, 6000);
    const b = { shownAt: 1000, hoverEndedAt: null };
    assert.equal(bubbleDeadline(b), 7000);
    assert.equal(shouldAutoDismiss({ ...b, now: 6999, hovering: false }), false);
    assert.equal(shouldAutoDismiss({ ...b, now: 7000, hovering: false }), true);
});

test('hovering keeps it, and leaving starts the TTL again', () => {
    const b = { shownAt: 1000, hoverEndedAt: null };
    assert.equal(shouldAutoDismiss({ ...b, now: 60000, hovering: true }), false);
    const left = { shownAt: 1000, hoverEndedAt: 20000 };
    assert.equal(bubbleDeadline(left), 26000);
    assert.equal(shouldAutoDismiss({ ...left, now: 25999, hovering: false }), false);
    assert.equal(shouldAutoDismiss({ ...left, now: 26000, hovering: false }), true);
});

test('a hover that ended before the TTL does not shorten it', () => {
    assert.equal(bubbleDeadline({ shownAt: 1000, hoverEndedAt: 500 }), 7000);
});

test('with a screen reader on, it never fades by itself', () => {
    const b = { shownAt: 0, hoverEndedAt: null, hovering: false, screenReader: true };
    assert.equal(shouldAutoDismiss({ ...b, now: 10 * BUBBLE_TTL_MS }), false);
});

// Mood -> eyes

test('each of the 5 moods has its own 3x3 eye', () => {
    assert.deepEqual(MOODS, ['concerned', 'sleepy', 'missed_you', 'calm', 'cheerful']);
    const seen = new Set();
    for (const mood of MOODS) {
        const grid = eyeGrid(mood);
        assert.equal(grid.length, 3, mood);
        for (const row of grid) {
            assert.equal(row.length, 3, mood);
            for (const cell of row) assert.ok(cell === 0 || cell === 1, mood);
        }
        seen.add(JSON.stringify(grid));
    }
    assert.equal(seen.size, 5);
});

test('calm is the eye the adult always had', () => {
    // White 6x6 with a black 4x4 pupil at the bottom right
    assert.deepEqual(eyeGrid('calm'), [
        [1, 1, 1],
        [1, 0, 0],
        [1, 0, 0],
    ]);
});

test('the mood shapes', () => {
    // concerned: looking down
    assert.deepEqual(eyeGrid('concerned'), [[1, 1, 1], [1, 1, 1], [1, 0, 1]]);
    // sleepy: lid over the top, half open
    assert.deepEqual(eyeGrid('sleepy'), [[0, 0, 0], [1, 1, 1], [1, 0, 1]]);
    // missed_you: a big pupil with one highlight
    assert.deepEqual(eyeGrid('missed_you'), [[1, 0, 0], [0, 0, 0], [0, 0, 0]]);
    // cheerful: an arc
    assert.deepEqual(eyeGrid('cheerful'), [[0, 1, 0], [1, 0, 1], [0, 0, 0]]);
});

test('an unknown mood reads as calm, and blink is a closed eye', () => {
    assert.deepEqual(eyeGrid('focused'), eyeGrid('calm'));
    assert.deepEqual(eyeGrid('blink'), [[0, 0, 0], [0, 0, 0], [1, 1, 1]]);
});

test('the face only shows for a living adult with the companion enabled', () => {
    assert.equal(faceMood(opened({ mood: 'sleepy' }), adult()), 'sleepy');
    assert.equal(faceMood(opened({ enabled: false, mood: 'sleepy' }), adult()), null);
    assert.equal(faceMood(null, adult()), null);
    assert.equal(faceMood(opened(), adult({ stage: 'Child' })), null);
    assert.equal(faceMood(opened(), adult({ is_alive: false })), null);
    assert.equal(faceMood(opened(), null), null);
});

test('a blink when the face appears or changes, not under reduced motion', () => {
    assert.equal(shouldBlink(null, 'calm', false), true);
    assert.equal(shouldBlink('calm', 'sleepy', false), true);
    assert.equal(shouldBlink('calm', 'calm', false), false);
    assert.equal(shouldBlink('calm', null, false), false);
    assert.equal(shouldBlink(null, 'missed_you', true), false);
});

// Info line

test('an idle adult shows the mood instead of content/hungry/tired/sad', () => {
    for (const status of ['content', 'hungry', 'tired', 'sad']) {
        assert.equal(statusText(adult({ status }), 'cheerful'), 'cheerful');
    }
    assert.equal(statusText(adult(), 'missed_you'), 'missed you');
    assert.equal(statusText(adult({ status: 'hungry' }), 'concerned'), 'concerned');
});

test('sleeping, eating and playing are shown over the mood', () => {
    const cases = [
        ['Sleeping', 'sleeping (1h 20m left)'],
        ['Eating', 'eating'],
        ['Playing', 'playing'],
    ];
    for (const [activity, status] of cases) {
        assert.equal(statusText(adult({ activity, status }), 'sleepy'), status);
    }
});

test('without a face the status is as before', () => {
    assert.equal(statusText(adult({ status: 'hungry' }), null), 'hungry');
    assert.equal(statusText(adult({ stage: 'Child', status: 'sad' }), 'calm'), 'sad');
    assert.equal(statusText(adult({ status: '' }), null), 'content');
});

// Focus

test('the first focus opens', () => {
    assert.equal(shouldCallOpen({ now: 5000, lastCallAt: null, inFlight: false }), true);
});

test('a focus right after the last one is ignored', () => {
    assert.equal(OPEN_MIN_GAP_MS, 2000);
    assert.equal(shouldCallOpen({ now: 6999, lastCallAt: 5000, inFlight: false }), false);
    assert.equal(shouldCallOpen({ now: 7000, lastCallAt: 5000, inFlight: false }), true);
});

test('no second call while one is still running', () => {
    assert.equal(shouldCallOpen({ now: 60000, lastCallAt: 5000, inFlight: true }), false);
});

test('a clock that went back does not block opening for good', () => {
    assert.equal(shouldCallOpen({ now: 1000, lastCallAt: 5000, inFlight: false }), true);
});

// Settings

test('quiet hours accept 0-23 only', () => {
    assert.equal(parseHour('0'), 0);
    assert.equal(parseHour('23'), 23);
    assert.equal(parseHour(' 7 '), 7);
    assert.equal(parseHour(7), 7);
    for (const bad of ['24', '-1', '7.5', '', 'x', null, undefined]) {
        assert.equal(parseHour(bad), null, String(bad));
    }
});

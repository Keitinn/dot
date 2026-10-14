import test from 'node:test';
import assert from 'node:assert/strict';
import {
    PATH_MAX,
    checkSyncPath,
    syncStatusRows,
    choiceLabel,
    whichDotChoices,
    shouldShowWhichDot,
} from '../frontend/js/sync-logic.js';

const clock = (secs) => `T${secs}`;
const on = (over = {}) => ({ enabled: true, last_sync_at: null, unreadable: 0, error: null, choice: null, ...over });
const pick = (over = {}) => ({ id: 'pet-x', stage: 'Adult', age: 12 * 86400, mine: true, ...over });

// Path input

test('a full path or one from the home folder is sent trimmed', () => {
    assert.deepEqual(checkSyncPath('  /Users/me/Dropbox/dot  '), { ok: true, path: '/Users/me/Dropbox/dot' });
    assert.deepEqual(checkSyncPath('~/Library/Mobile Documents/com~apple~CloudDocs/dot'), {
        ok: true,
        path: '~/Library/Mobile Documents/com~apple~CloudDocs/dot',
    });
    assert.deepEqual(checkSyncPath('~'), { ok: true, path: '~' });
});

test('an empty or relative path is refused before it is sent', () => {
    assert.deepEqual(checkSyncPath(''), { ok: false, error: 'ENTER A FOLDER' });
    assert.deepEqual(checkSyncPath('   '), { ok: false, error: 'ENTER A FOLDER' });
    assert.deepEqual(checkSyncPath(null), { ok: false, error: 'ENTER A FOLDER' });
    assert.deepEqual(checkSyncPath('Dropbox/dot'), { ok: false, error: 'USE A FULL PATH' });
    assert.deepEqual(checkSyncPath('~me/dot'), { ok: false, error: 'USE A FULL PATH' });
});

test('a path with a NUL or longer than the limit is refused', () => {
    assert.deepEqual(checkSyncPath('/a\0b'), { ok: false, error: 'NOT A PATH' });
    assert.deepEqual(checkSyncPath('/' + 'a'.repeat(PATH_MAX)), { ok: false, error: 'PATH TOO LONG' });
    assert.equal(checkSyncPath('/' + 'a'.repeat(PATH_MAX - 1)).ok, true);
});

// Status line

test('off reads OFF, whatever else is there', () => {
    assert.deepEqual(syncStatusRows({ enabled: false, last_sync_at: 5, error: 'X' }, clock), ['OFF']);
    assert.deepEqual(syncStatusRows(null, clock), []);
});

test('on shows when it last synced, or that it has not yet', () => {
    assert.deepEqual(syncStatusRows(on({ last_sync_at: 600 }), clock), ['LAST SYNC T600']);
    assert.deepEqual(syncStatusRows(on(), clock), ['NOT SYNCED YET']);
});

test('an error comes first and the last sync stays below it', () => {
    assert.deepEqual(syncStatusRows(on({ error: 'FOLDER MISSING', last_sync_at: 60 }), clock), [
        'FOLDER MISSING',
        'LAST SYNC T60',
    ]);
});

test('skipped files are counted', () => {
    assert.deepEqual(syncStatusRows(on({ last_sync_at: 60, unreadable: 1 }), clock), ['LAST SYNC T60', '1 FILE SKIPPED']);
    assert.deepEqual(syncStatusRows(on({ last_sync_at: 60, unreadable: 3 }), clock), ['LAST SYNC T60', '3 FILES SKIPPED']);
});

test('the default clock is hh:mm', () => {
    const at = new Date(2026, 9, 14, 9, 5).getTime() / 1000;
    assert.deepEqual(syncStatusRows(on({ last_sync_at: at })), ['LAST SYNC 09:05']);
});

// WHICH DOT?

test('a choice reads stage and age, in hours under a day', () => {
    assert.equal(choiceLabel(pick()), 'ADULT 12D HERE');
    assert.equal(choiceLabel(pick({ stage: 'Child', age: 3 * 3600 + 59, mine: false })), 'CHILD 3H');
    assert.equal(choiceLabel(pick({ stage: 'Egg', age: 0, mine: false })), 'EGG 0H');
});

test('the choices are listed with this Mac first', () => {
    const status = on({ choice: [pick({ id: 'b', mine: false }), pick({ id: 'a', mine: true })] });
    assert.deepEqual(
        whichDotChoices(status).map((c) => [c.id, c.mine]),
        [['a', true], ['b', false]],
    );
    assert.deepEqual(whichDotChoices(on()), []);
    assert.deepEqual(whichDotChoices(null), []);
});

test('WHICH DOT? shows only with sync on and two pets or more, until LATER', () => {
    const two = on({ choice: [pick(), pick({ id: 'y', mine: false })] });
    assert.equal(shouldShowWhichDot(two, false), true);
    assert.equal(shouldShowWhichDot(two, true), false);
    assert.equal(shouldShowWhichDot({ ...two, enabled: false }, false), false);
    assert.equal(shouldShowWhichDot(on({ choice: [pick()] }), false), false);
    assert.equal(shouldShowWhichDot(on(), false), false);
    assert.equal(shouldShowWhichDot(null, false), false);
});

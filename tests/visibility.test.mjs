import test from 'node:test';
import assert from 'node:assert/strict';
import { shouldRun } from '../frontend/js/visibility.js';

test('shouldRun: only a shown, focused popover draws and polls', () => {
    assert.equal(shouldRun({ hidden: false, focused: true }), true);
});

test('shouldRun: a shown window without focus is about to hide', () => {
    assert.equal(shouldRun({ hidden: false, focused: false }), false);
});

test('shouldRun: a hidden window never runs', () => {
    assert.equal(shouldRun({ hidden: true, focused: false }), false);
    assert.equal(shouldRun({ hidden: true, focused: true }), false);
});

// The window is moved by a drag strip, and that is the only window permission it gets.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), 'utf8');

test('index.html has one drag region, outside the buttons', () => {
    const html = read('frontend/index.html');
    assert.equal(html.match(/data-tauri-drag-region/g).length, 1);
    const region = html.indexOf('id="drag-region"');
    assert.ok(region >= 0 && region < html.indexOf('id="settings-container"'));
});

test('the drag strip sits below the buttons', () => {
    const css = read('frontend/styles/main.css');
    const z = (id) => Number(new RegExp(`#${id} \\{[^}]*z-index: (\\d+)`).exec(css)[1]);
    assert.ok(z('drag-region') < z('settings-container'));
});

test('capability: main window, start-dragging only', () => {
    const cap = JSON.parse(read('backend/capabilities/default.json'));
    assert.deepEqual(cap.windows, ['main']);
    assert.deepEqual(cap.permissions, ['core:window:allow-start-dragging']);
});

test('the Dock icon is hidden by the activation policy, not only by Info.plist', () => {
    const main = read('backend/src/main.rs');
    assert.match(main, /set_activation_policy\(tauri::ActivationPolicy::Accessory\)/);
    assert.match(read('backend/Info.plist'), /<key>LSUIElement<\/key>\s*<true\/>/);
});

// The bundle ID and the release workflow's signing steps. A wrong ID or a leaked secret is costly to undo.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), 'utf8');

test('bundle ID is the registered App ID', () => {
    const conf = JSON.parse(read('backend/tauri.conf.json'));
    assert.equal(conf.identifier, 'io.github.keitinn.dot');
});

test('release workflow: signed builds are gated on the certificate secret, unsigned builds stay', () => {
    const yml = read('.github/workflows/release.yml');
    assert.match(yml, /steps\.signing\.outputs\.enabled == 'true'/);
    assert.match(yml, /steps\.signing\.outputs\.enabled != 'true'/);
    for (const name of ['APPLE_CERTIFICATE', 'APPLE_CERTIFICATE_PASSWORD', 'APPLE_SIGNING_IDENTITY',
        'APPLE_API_ISSUER', 'APPLE_API_KEY', 'APPLE_API_KEY_P8']) {
        assert.ok(yml.includes(`secrets.${name}`), `${name} is read from secrets`);
    }
});

test('release workflow: secrets are never echoed or written to the log', () => {
    const yml = read('.github/workflows/release.yml');
    for (const line of yml.split('\n')) {
        if (/^\s*(echo|printf|cat)\b/.test(line)) {
            assert.ok(!/APPLE_(CERTIFICATE|API_KEY_P8)\b/.test(line.replace(/"\$APPLE_API_KEY_P8" >/, '')),
                `a secret value is printed: ${line.trim()}`);
        }
    }
    assert.ok(!/secrets\.[A-Z_]+\s*\}\}\s*['"]?\s*>>\s*"?\$GITHUB_(ENV|OUTPUT)/.test(yml));
});

test('release workflow: verifies signature and notarization after a signed build', () => {
    const yml = read('.github/workflows/release.yml');
    for (const cmd of ['codesign --verify', 'spctl --assess', 'stapler validate']) {
        assert.ok(yml.includes(cmd), `${cmd} runs in the release workflow`);
    }
});

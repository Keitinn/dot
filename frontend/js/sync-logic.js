// Pure rules for the SYNC settings and WHICH DOT? (#38, design「複数端末とセーブ同期」).
// No DOM here, so node --test can check them. The backend checks the folder
// itself; this only catches what is wrong before it is sent.

export const PATH_MAX = 1024;

/** The folder typed in SYNC, trimmed, or why it cannot be sent. */
export function checkSyncPath(text) {
    const path = typeof text === 'string' ? text.trim() : '';
    if (!path) return { ok: false, error: 'ENTER A FOLDER' };
    if (path.includes('\0')) return { ok: false, error: 'NOT A PATH' };
    if (path.length > PATH_MAX) return { ok: false, error: 'PATH TOO LONG' };
    if (!(path.startsWith('/') || path === '~' || path.startsWith('~/'))) {
        return { ok: false, error: 'USE A FULL PATH' };
    }
    return { ok: true, path };
}

function hhmm(unixSecs) {
    const d = new Date(unixSecs * 1000);
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** The rows under SYNC for a `get_sync_status` result. */
export function syncStatusRows(status, clock = hhmm) {
    if (!status) return [];
    if (!status.enabled) return ['OFF'];
    const rows = [];
    if (status.error) rows.push(status.error);
    rows.push(status.last_sync_at ? `LAST SYNC ${clock(status.last_sync_at)}` : 'NOT SYNCED YET');
    const skipped = status.unreadable || 0;
    if (skipped > 0) rows.push(`${skipped} FILE${skipped === 1 ? '' : 'S'} SKIPPED`);
    return rows;
}

/** One pet at WHICH DOT?: its stage and age, and HERE for this Mac's. */
export function choiceLabel(choice) {
    const age = Math.max(0, Math.floor(choice.age || 0));
    const shown = age >= 86400 ? `${Math.floor(age / 86400)}D` : `${Math.floor(age / 3600)}H`;
    const label = `${String(choice.stage || '').toUpperCase()} ${shown}`;
    return choice.mine ? `${label} HERE` : label;
}

/** The pets to choose from, this Mac's first. */
export function whichDotChoices(status) {
    if (!status || !Array.isArray(status.choice)) return [];
    return [...status.choice].sort((a, b) => Number(Boolean(b.mine)) - Number(Boolean(a.mine)));
}

/** WHICH DOT? waits in the popover until a pet is chosen; LATER hides it until the next open. */
export function shouldShowWhichDot(status, dismissed) {
    if (!status || !status.enabled || dismissed) return false;
    return whichDotChoices(status).length >= 2;
}

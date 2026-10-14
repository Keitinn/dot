// Pure rules for the companion UI (design/gdd/companion.md R1–R4, UI Requirements).
// No DOM here, so node --test can check them.

export const BUBBLE_TTL_MS = 6000;
export const LINE_MAX_CHARS = 24;
// Up to this many characters fit on one row of the bubble.
export const ONE_ROW_MAX = 14;
// A focus this soon after the last open is the same open.
export const OPEN_MIN_GAP_MS = 2000;

export const MOODS = ['concerned', 'sleepy', 'missed_you', 'calm', 'cheerful'];

export const ONBOARDING_TEXT = 'STAY WITH YOU?';

/** The line as at most 2 rows, 24 characters in all. */
export function splitLine(text) {
    if (typeof text !== 'string') return [];
    const line = text.replace(/\s+/g, ' ').trim().slice(0, LINE_MAX_CHARS).trim();
    if (!line) return [];
    if (line.length <= ONE_ROW_MAX) return [line];

    let best = null;
    for (let i = line.indexOf(' '); i !== -1; i = line.indexOf(' ', i + 1)) {
        const rows = [line.slice(0, i), line.slice(i + 1)];
        const width = Math.max(rows[0].length, rows[1].length);
        if (!best || width < best.width) best = { rows, width };
    }
    if (best) return best.rows;
    const half = Math.ceil(line.length / 2);
    return [line.slice(0, half), line.slice(half)];
}

/** What the bubble shows for a `companion_open` result, or null for nothing. */
export function bubbleContent(result) {
    if (!result || !result.enabled) return null;
    if (result.onboarding === 'prompt') {
        return { kind: 'prompt', rows: [ONBOARDING_TEXT], answers: ['yes', 'later'] };
    }
    const rows = splitLine(result.line && result.line.text);
    return rows.length ? { kind: 'line', rows } : null;
}

/** When the bubble fades: TTL after it was shown or after the pointer left it. */
export function bubbleDeadline({ shownAt, hoverEndedAt }) {
    return Math.max(shownAt, hoverEndedAt ?? shownAt) + BUBBLE_TTL_MS;
}

export function shouldAutoDismiss({ now, shownAt, hoverEndedAt, hovering, screenReader = false }) {
    if (screenReader || hovering) return false;
    return now >= bubbleDeadline({ shownAt, hoverEndedAt });
}

// 3x3 cells of 2x2 pixels inside each 6x6 eye: 1 = screen colour, 0 = black.
const EYES = {
    calm: [[1, 1, 1], [1, 0, 0], [1, 0, 0]],
    concerned: [[1, 1, 1], [1, 1, 1], [1, 0, 1]],
    sleepy: [[0, 0, 0], [1, 1, 1], [1, 0, 1]],
    missed_you: [[1, 0, 0], [0, 0, 0], [0, 0, 0]],
    cheerful: [[0, 1, 0], [1, 0, 1], [0, 0, 0]],
    blink: [[0, 0, 0], [0, 0, 0], [1, 1, 1]],
};

/** The eye for a mood, or 'blink'. Unknown moods read as calm, as on the backend. */
export function eyeGrid(mood) {
    return EYES[mood] || EYES.calm;
}

/** The mood to draw, or null to draw the adult as it always was. */
export function faceMood(result, pet) {
    if (!result || !result.enabled || !pet) return null;
    if (pet.stage !== 'Adult' || !pet.is_alive) return null;
    return MOODS.includes(result.mood) ? result.mood : 'calm';
}

export function shouldBlink(prevMood, nextMood, reducedMotion) {
    return !reducedMotion && nextMood !== null && nextMood !== prevMood;
}

/** The status in the info line; sleeping/eating/playing come before the mood. */
export function statusText(pet, mood) {
    const status = (pet && pet.status) || 'content';
    if (!mood || !pet || pet.stage !== 'Adult' || pet.activity !== 'Idle') return status;
    return mood.replace(/_/g, ' ');
}

/** Whether a window focus is a new open of the popover. */
export function shouldCallOpen({ now, lastCallAt, inFlight }) {
    if (inFlight) return false;
    if (lastCallAt === null || lastCallAt === undefined) return true;
    // A clock that went back counts as long ago
    return now < lastCallAt || now - lastCallAt >= OPEN_MIN_GAP_MS;
}

/** A quiet-hours hour from the settings field, or null if it is not 0–23. */
export function parseHour(value) {
    if (value === null || value === undefined) return null;
    const text = String(value).trim();
    if (!/^\d{1,2}$/.test(text)) return null;
    const hour = Number(text);
    return hour <= 23 ? hour : null;
}

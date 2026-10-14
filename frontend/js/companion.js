import {
    bubbleContent,
    bubbleDeadline,
    faceMood,
    parseHour,
    shouldAutoDismiss,
    shouldCallOpen,
} from './companion-logic.js';

const FADE_MS = 400;

function reducedMotion() {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}

function el(tag, className, text) {
    const e = document.createElement(tag);
    if (className) e.className = className;
    if (text !== undefined) e.textContent = text;
    return e;
}

function clock(unixSecs) {
    const d = new Date(unixSecs * 1000);
    return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/**
 * The adult companion in the popover: the mood on the eyes, one line in a
 * bubble, STAY WITH YOU?, and the COMPANION settings (design/gdd/companion.md).
 */
export class CompanionUI {
    constructor(api, renderer, getPet, onMoodChange) {
        this.api = api;
        this.renderer = renderer;
        this.getPet = getPet;
        this.onMoodChange = onMoodChange || (() => {});

        // The last companion_open result, its mood kept fresh by refreshMood
        this.result = null;
        this.lastOpenAt = null;
        this.opening = false;
        this.refreshing = false;

        this.live = document.getElementById('companion-bubble');
        this.bubble = null;
        this.bubbleTimes = null;
        this.hovering = false;
        this.fadeTimer = null;

        // The window hides when it loses focus, so each focus is a new open
        window.addEventListener('focus', () => this.open());
        window.addEventListener('blur', () => this.dismiss());
        document.addEventListener('visibilitychange', () => {
            if (document.hidden) this.dismiss();
        });

        this.settings = null;
        this.initSettings();
    }

    /** For a window that already has focus when the app starts. */
    start() {
        if (document.hasFocus()) this.open();
    }

    /** The mood on the face now, or null when the companion is off. */
    mood() {
        return faceMood(this.result, this.getPet());
    }

    /** Puts the current mood (or none) on the renderer. */
    syncFace() {
        this.renderer.setFace(this.mood(), { reducedMotion: reducedMotion() });
    }

    async open() {
        const now = performance.now();
        if (!shouldCallOpen({ now, lastCallAt: this.lastOpenAt, inFlight: this.opening })) return;
        this.lastOpenAt = now;
        this.opening = true;
        let result;
        try {
            result = await this.api.companionOpen();
        } catch (error) {
            console.error('Failed to open companion:', error);
            return;
        } finally {
            this.opening = false;
        }
        this.result = result;
        this.dismiss();
        this.onMoodChange();
        // Closed again before the answer came: the line is dropped, not kept for later
        if (document.hasFocus()) this.showBubble(bubbleContent(result));
    }

    /** Called with each poll. Updates the mood while the popover stays open. */
    async refreshMood() {
        if (this.refreshing || this.opening || !this.mood() || !document.hasFocus()) return;
        this.refreshing = true;
        try {
            const { enabled, mood } = await this.api.refreshCompanionMood();
            if (this.result && (enabled !== this.result.enabled || mood !== this.result.mood)) {
                this.result = { ...this.result, enabled, mood };
                this.onMoodChange();
            }
        } catch (error) {
            console.error('Failed to refresh companion mood:', error);
        } finally {
            this.refreshing = false;
        }
    }

    // Bubble

    showBubble(content) {
        if (!content) return;
        const bubble = el('div', 'companion-bubble');
        const text = el('div', 'bubble-text');
        content.rows.forEach((row, i) => {
            if (i > 0) text.appendChild(document.createElement('br'));
            text.appendChild(document.createTextNode(row));
        });
        bubble.appendChild(text);

        if (content.kind === 'prompt') {
            const answers = el('div', 'bubble-answers');
            for (const answer of content.answers) {
                const b = el('button', 'bubble-btn', answer.toUpperCase());
                b.type = 'button';
                b.addEventListener('click', (e) => {
                    e.stopPropagation();
                    this.answer(answer);
                });
                answers.appendChild(b);
            }
            bubble.appendChild(answers);
        }

        bubble.addEventListener('click', () => this.dismiss());
        const hold = () => {
            this.hovering = true;
            clearTimeout(this.fadeTimer);
            bubble.classList.remove('fading');
        };
        const release = () => {
            this.hovering = false;
            if (this.bubbleTimes) this.bubbleTimes.hoverEndedAt = performance.now();
            this.scheduleFade();
        };
        bubble.addEventListener('mouseenter', hold);
        bubble.addEventListener('mouseleave', release);
        bubble.addEventListener('focusin', hold);
        bubble.addEventListener('focusout', release);

        this.bubble = bubble;
        this.hovering = false;
        this.bubbleTimes = { shownAt: performance.now(), hoverEndedAt: null };
        this.live.appendChild(bubble);
        this.scheduleFade();
    }

    scheduleFade() {
        clearTimeout(this.fadeTimer);
        if (!this.bubble || this.hovering) return;
        const now = performance.now();
        // There is no way to tell from the webview that VoiceOver is on, so it fades as usual
        if (shouldAutoDismiss({ now, ...this.bubbleTimes, hovering: false, screenReader: false })) {
            this.fade();
            return;
        }
        this.fadeTimer = setTimeout(() => this.scheduleFade(), bubbleDeadline(this.bubbleTimes) - now);
    }

    fade() {
        const bubble = this.bubble;
        if (!bubble) return;
        if (reducedMotion()) {
            this.dismiss();
            return;
        }
        bubble.classList.add('fading');
        this.fadeTimer = setTimeout(() => {
            if (this.bubble === bubble && !this.hovering) this.dismiss();
        }, FADE_MS);
    }

    /** Removes the bubble at once; nothing is kept unread. */
    dismiss() {
        clearTimeout(this.fadeTimer);
        if (this.bubble) this.bubble.remove();
        this.bubble = null;
        this.bubbleTimes = null;
        this.hovering = false;
    }

    async answer(answer) {
        this.dismiss();
        try {
            await this.api.companionOnboardingAnswer(answer);
        } catch (error) {
            console.error('Failed to answer onboarding:', error);
        }
    }

    // Settings (COMPANION in the * menu)

    initSettings() {
        this.menuBtn = document.getElementById('companion-menu-btn');
        this.panel = document.getElementById('companion-settings');
        this.masterBtn = document.getElementById('companion-master-btn');
        this.quietStart = document.getElementById('quiet-start');
        this.quietEnd = document.getElementById('quiet-end');
        this.quietNote = document.getElementById('companion-quiet-note');

        // Clicks inside must not close the * menu
        this.panel.addEventListener('click', (e) => e.stopPropagation());
        this.menuBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            this.expandSettings(this.panel.classList.contains('hidden'));
        });
        // Folded again whenever the * menu opens or closes, so it is never stale
        document.getElementById('settings-btn').addEventListener('click', () => this.expandSettings(false));
        document.addEventListener('click', () => this.expandSettings(false));

        this.masterBtn.addEventListener('click', () => {
            if (this.settings) this.saveSettings({ master_on: !this.settings.master_on });
        });
        document.getElementById('quiet-1h-btn').addEventListener('click', () => this.saveSettings({ quiet: 'quiet_1h' }));
        document.getElementById('quiet-today-btn').addEventListener('click', () => this.saveSettings({ quiet: 'quiet_today' }));
        document.getElementById('quiet-off-btn').addEventListener('click', () => this.saveSettings({ quiet: 'off' }));

        const hourField = (input, key) => {
            input.addEventListener('change', () => {
                const hour = parseHour(input.value);
                if (hour === null) {
                    this.renderSettings();
                    return;
                }
                this.saveSettings({ [key]: hour });
            });
        };
        hourField(this.quietStart, 'quiet_start');
        hourField(this.quietEnd, 'quiet_end');
    }

    expandSettings(open) {
        this.panel.classList.toggle('hidden', !open);
        this.menuBtn.setAttribute('aria-expanded', String(open));
        if (open) this.loadSettings();
    }

    async loadSettings() {
        try {
            this.settings = await this.api.getCompanionSettings();
        } catch (error) {
            console.error('Failed to get companion settings:', error);
            return;
        }
        this.renderSettings();
    }

    async saveSettings(update) {
        try {
            this.settings = await this.api.setCompanionSettings(update);
        } catch (error) {
            console.error('Failed to save companion settings:', error);
        }
        // Turning it off or going quiet takes the current line away too
        if (this.settings && (!this.settings.master_on || this.settings.quiet_until)) this.dismiss();
        this.renderSettings();
    }

    renderSettings() {
        const s = this.settings;
        if (!s) return;
        // The label stays ON; aria-pressed (drawn as [X] / [ ]) carries the state
        this.masterBtn.setAttribute('aria-pressed', String(s.master_on));
        this.quietStart.value = String(s.quiet_start);
        this.quietEnd.value = String(s.quiet_end);
        this.quietNote.textContent = s.quiet_until ? `QUIET TILL ${clock(s.quiet_until)}` : '';
    }
}

import {
    checkSyncPath,
    choiceLabel,
    shouldShowWhichDot,
    syncStatusRows,
    whichDotChoices,
} from './sync-logic.js';

function el(tag, className, text) {
    const e = document.createElement(tag);
    if (className) e.className = className;
    if (text !== undefined) e.textContent = text;
    return e;
}

function setRows(target, rows) {
    target.textContent = '';
    rows.forEach((row, i) => {
        if (i > 0) target.appendChild(document.createElement('br'));
        target.appendChild(document.createTextNode(row));
    });
}

/**
 * SYNC in the * menu, and WHICH DOT? over the pet (#38). The backend owns
 * the folder and the merge; this only shows them and passes on the clicks.
 */
export class SyncUI {
    constructor(api, onChanged) {
        this.api = api;
        // The pet or the coins may have changed: the app fetches them again
        this.onChanged = onChanged || (() => {});

        this.settings = null;
        this.status = null;
        // A refusal (an invalid folder, a failed call), shown until the next action
        this.note = null;
        this.busy = false;
        this.refreshing = false;
        this.choosing = false;
        // LATER hides WHICH DOT? until the popover opens again
        this.dismissed = false;
        this.choiceKey = null;

        this.choiceEl = document.getElementById('sync-choice');
        // Registered before the app's focus handler, whose refresh then shows it again
        window.addEventListener('focus', () => {
            this.dismissed = false;
        });
        this.initSettings();
    }

    initSettings() {
        this.menuBtn = document.getElementById('sync-menu-btn');
        this.panel = document.getElementById('sync-settings');
        this.enabledBtn = document.getElementById('sync-enabled-btn');
        this.pathInput = document.getElementById('sync-path');
        this.setBtn = document.getElementById('sync-set-btn');
        this.nowBtn = document.getElementById('sync-now-btn');
        this.statusEl = document.getElementById('sync-status');

        // Clicks inside must not close the * menu
        this.panel.addEventListener('click', (e) => e.stopPropagation());
        this.menuBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            this.expand(this.panel.classList.contains('hidden'));
        });
        // Folded again whenever the * menu opens or closes, as COMPANION is
        document.getElementById('settings-btn').addEventListener('click', () => this.expand(false));
        document.addEventListener('click', () => this.expand(false));

        this.enabledBtn.addEventListener('click', () => {
            if (this.settings) this.save({ enabled: !this.settings.enabled });
        });
        this.setBtn.addEventListener('click', () => this.setFolder());
        this.pathInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') this.setFolder();
        });
        this.nowBtn.addEventListener('click', () => this.syncNow());
    }

    expand(open) {
        this.panel.classList.toggle('hidden', !open);
        this.menuBtn.setAttribute('aria-expanded', String(open));
        if (open) {
            this.note = null;
            this.load();
        }
    }

    async load() {
        try {
            const [settings, status] = await Promise.all([this.api.getSyncSettings(), this.api.getSyncStatus()]);
            this.settings = settings;
            this.status = status;
        } catch (error) {
            console.error('Failed to get sync settings:', error);
            return;
        }
        this.pathInput.value = this.settings.path || '';
        this.renderSettings();
        this.renderChoice();
    }

    setFolder() {
        const check = checkSyncPath(this.pathInput.value);
        if (!check.ok) {
            this.note = check.error;
            this.renderSettings();
            return;
        }
        this.save({ path: check.path });
    }

    /** `update` is { path?, enabled? }; the backend refuses an invalid folder and keeps the old one. */
    async save(update) {
        if (this.busy) return;
        this.busy = true;
        this.renderSettings();
        try {
            this.settings = await this.api.setSyncSettings(update);
            this.note = null;
            this.pathInput.value = this.settings.path || '';
            this.status = await this.api.getSyncStatus();
            this.onChanged();
        } catch (error) {
            this.note = String(error);
        } finally {
            this.busy = false;
        }
        this.renderSettings();
        this.renderChoice();
    }

    async syncNow() {
        if (this.busy) return;
        this.busy = true;
        this.renderSettings();
        try {
            this.status = await this.api.syncNow();
            this.note = null;
            this.onChanged();
        } catch (error) {
            this.note = String(error);
        } finally {
            this.busy = false;
        }
        this.renderSettings();
        this.renderChoice();
    }

    /** Called with each poll: the status, and WHICH DOT? when there is a choice. */
    async refresh() {
        if (this.refreshing) return;
        this.refreshing = true;
        try {
            this.status = await this.api.getSyncStatus();
        } catch (error) {
            console.error('Failed to get sync status:', error);
            return;
        } finally {
            this.refreshing = false;
        }
        if (!this.panel.classList.contains('hidden')) this.renderSettings();
        this.renderChoice();
    }

    renderSettings() {
        if (!this.settings) return;
        // The label stays SYNC FOLDER; aria-pressed (drawn as [X] / [ ]) carries the state
        this.enabledBtn.setAttribute('aria-pressed', String(this.settings.enabled));
        this.enabledBtn.disabled = this.busy;
        this.setBtn.disabled = this.busy;
        this.nowBtn.disabled = this.busy || !this.settings.enabled;
        const rows = syncStatusRows(this.status || { enabled: this.settings.enabled });
        setRows(this.statusEl, this.note ? [this.note, ...rows] : rows);
    }

    renderChoice() {
        const show = shouldShowWhichDot(this.status, this.dismissed);
        const choices = show ? whichDotChoices(this.status) : [];
        const key = JSON.stringify(choices.map((c) => [c.id, choiceLabel(c)]));
        // Rebuilt only when it changes, so a poll never takes the focus away
        if (key === this.choiceKey) return;
        this.choiceKey = key;
        this.choiceEl.textContent = '';
        this.choiceEl.hidden = !show;
        if (!show) return;

        const bubble = el('div', 'sync-choice-bubble');
        bubble.appendChild(el('div', 'bubble-text', 'WHICH DOT?'));
        const answers = el('div', 'sync-choice-answers');
        for (const choice of choices) {
            const b = el('button', 'bubble-btn', choiceLabel(choice));
            b.type = 'button';
            b.addEventListener('click', (e) => {
                e.stopPropagation();
                this.choose(choice.id);
            });
            answers.appendChild(b);
        }
        const later = el('button', 'bubble-btn sync-choice-later', 'LATER');
        later.type = 'button';
        later.addEventListener('click', (e) => {
            e.stopPropagation();
            this.dismissed = true;
            this.renderChoice();
        });
        answers.appendChild(later);
        bubble.appendChild(answers);
        this.choiceEl.appendChild(bubble);
    }

    async choose(petId) {
        if (this.choosing) return;
        this.choosing = true;
        try {
            this.status = await this.api.syncChoosePet(petId);
            this.onChanged();
        } catch (error) {
            console.error('Failed to choose the pet:', error);
        } finally {
            this.choosing = false;
        }
        this.renderChoice();
    }
}

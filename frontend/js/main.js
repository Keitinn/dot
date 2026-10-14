import { PetRenderer } from './renderer.js';
import { PetAPI } from './api.js';
import { GamesUI } from './games.js';
import { ShopUI } from './shop.js';

const FEED_COOLDOWN_SECS = 1800; // 30 minutes
const PLAY_COOLDOWN_SECS = 1200; // 20 minutes

class DotApp {
    constructor() {
        this.renderer = new PetRenderer('pet-canvas');
        this.api = new PetAPI();
        this.petState = null;
        this.gameStats = null;
        this.updateInterval = null;
        this.toastTimeout = null;

        this.init();
    }

    async init() {
        // Set up button event listeners
        document.getElementById('feed-btn').addEventListener('click', () => this.feedPet());
        document.getElementById('play-btn').addEventListener('click', () => this.playWithPet());
        document.getElementById('sleep-btn').addEventListener('click', () => this.putToSleep());
        document.getElementById('revive-btn').addEventListener('click', () => this.revivePet());

        this.games = new GamesUI(this.api, (r) => this.setCoins(r.coins));
        this.shop = new ShopUI(
            this.api,
            () => (this.gameStats ? this.gameStats.coins : 0),
            (result) => this.handleActionResult(result)
        );

        // Trophy / achievements button
        document.getElementById('trophy-btn').addEventListener('click', (e) => {
            e.stopPropagation();
            this.toggleAchievements();
        });
        document.getElementById('achievements-close-btn').addEventListener('click', () => {
            document.getElementById('achievements-overlay').classList.add('hidden');
        });

        // Settings menu
        const settingsBtn = document.getElementById('settings-btn');
        const settingsMenu = document.getElementById('settings-menu');
        settingsBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            settingsMenu.classList.toggle('hidden');
        });
        document.getElementById('reset-btn').addEventListener('click', () => {
            this.revivePet();
            settingsMenu.classList.add('hidden');
        });
        document.getElementById('quit-btn').addEventListener('click', () => this.api.quitApp());
        document.addEventListener('click', () => settingsMenu.classList.add('hidden'));

        // Load initial state
        await this.updatePetState();
        await this.fetchStats();
        await this.showLoadWarning();

        // Start render loop
        this.renderer.start((deltaTime) => {
            if (this.petState) {
                this.renderer.render(this.petState);
            }
        });

        // Poll for state updates every 5 seconds
        this.updateInterval = setInterval(() => {
            this.updatePetState();
            this.fetchStats();
        }, 5000);
    }

    async updatePetState() {
        try {
            this.petState = await this.api.getPetState();
            this.updateUI();
        } catch (error) {
            console.error('Failed to get pet state:', error);
        }
    }

    async fetchStats() {
        try {
            this.gameStats = await this.api.getGameStats();
            this.updateStreakDisplay();
            this.updateCoinDisplay();
        } catch (error) {
            console.error('Failed to get game stats:', error);
        }
    }

    updateUI() {
        if (!this.petState) return;

        // Update stat bars
        this.updateStatBar('hunger', this.petState.hunger);
        this.updateStatBar('happiness', this.petState.happiness);
        this.updateStatBar('energy', this.petState.energy);
        this.updateStatBar('health', this.petState.health);

        // Update pet info
        document.getElementById('pet-name').textContent = this.petState.name || 'dot';
        document.getElementById('pet-stage').textContent = this.petState.stage;

        // Calculate age in days
        const ageDays = Math.floor(this.petState.age / 86400);
        document.getElementById('pet-age').textContent = `${ageDays}d`;

        // Update status text
        document.getElementById('pet-status').textContent = this.petState.status || 'content';

        // Handle death state
        const deathOverlay = document.getElementById('death-overlay');

        if (!this.petState.is_alive) {
            deathOverlay.classList.remove('hidden');
            this.setButtonStates(true, true, true);
        } else {
            deathOverlay.classList.add('hidden');
            this.updateButtonStates();
        }
    }

    updateButtonStates() {
        if (!this.petState) return;

        const isSleeping = this.petState.activity === 'Sleeping';
        const now = Math.floor(Date.now() / 1000);

        const feedBtn = document.getElementById('feed-btn');
        const playBtn = document.getElementById('play-btn');
        const sleepBtn = document.getElementById('sleep-btn');

        if (isSleeping) {
            // All buttons disabled while sleeping, show ZZZ on sleep
            feedBtn.disabled = true;
            feedBtn.textContent = 'FEED';
            playBtn.disabled = true;
            playBtn.textContent = 'PLAY';
            sleepBtn.disabled = true;
            sleepBtn.textContent = 'ZZZ';
            return;
        }

        // Feed cooldown
        if (this.petState.last_feed_time) {
            const elapsed = now - this.petState.last_feed_time;
            const remaining = FEED_COOLDOWN_SECS - elapsed;
            if (remaining > 0) {
                const mins = Math.ceil(remaining / 60);
                feedBtn.disabled = true;
                feedBtn.textContent = `FEED (${mins}m)`;
            } else {
                feedBtn.disabled = false;
                feedBtn.textContent = 'FEED';
            }
        } else {
            feedBtn.disabled = false;
            feedBtn.textContent = 'FEED';
        }

        // Play cooldown
        if (this.petState.last_play_time) {
            const elapsed = now - this.petState.last_play_time;
            const remaining = PLAY_COOLDOWN_SECS - elapsed;
            if (remaining > 0) {
                const mins = Math.ceil(remaining / 60);
                playBtn.disabled = true;
                playBtn.textContent = `PLAY (${mins}m)`;
            } else {
                playBtn.disabled = false;
                playBtn.textContent = 'PLAY';
            }
        } else {
            playBtn.disabled = false;
            playBtn.textContent = 'PLAY';
        }

        // Sleep button
        sleepBtn.disabled = false;
        sleepBtn.textContent = 'SLEEP';
    }

    setButtonStates(feedDisabled, playDisabled, sleepDisabled) {
        document.getElementById('feed-btn').disabled = feedDisabled;
        document.getElementById('play-btn').disabled = playDisabled;
        document.getElementById('sleep-btn').disabled = sleepDisabled;
    }

    setCoins(coins) {
        if (!this.gameStats) return;
        this.gameStats.coins = coins;
        this.updateCoinDisplay();
    }

    updateCoinDisplay() {
        if (!this.gameStats) return;
        document.getElementById('coin-label').textContent = `$${this.gameStats.coins}`;
        if (this.shop) this.shop.render();
    }

    updateStreakDisplay() {
        if (!this.gameStats) return;
        const streakEl = document.getElementById('pet-streak');
        streakEl.textContent = `${this.gameStats.current_streak} streak`;
    }

    updateStatBar(stat, value) {
        const bar = document.getElementById(`${stat}-bar`);
        const valueSpan = document.getElementById(`${stat}-value`);

        bar.style.width = `${value}%`;
        valueSpan.textContent = value;

        // Keep black and white - no color changes
        bar.style.background = '#000';
    }

    handleActionResult(result) {
        this.petState = result;
        this.updateUI();
        this.fetchStats();

        // Show toast for new achievements
        if (result.new_achievements && result.new_achievements.length > 0) {
            for (const achievement of result.new_achievements) {
                this.showAchievementToast(achievement.name);
            }
        }
    }

    // The window starts hidden, so wait until the user first opens it.
    async showLoadWarning() {
        let warning;
        try {
            warning = await this.api.getLoadWarning();
        } catch (error) {
            console.error('Failed to get load warning:', error);
            return;
        }
        if (!warning) return;

        const show = () => this.showToast(warning, 6000);
        if (document.hasFocus()) {
            show();
        } else {
            window.addEventListener('focus', show, { once: true });
        }
    }

    showAchievementToast(name) {
        this.showToast(`UNLOCKED: ${name}`);
    }

    showToast(message, durationMs = 2500) {
        const toast = document.getElementById('achievement-toast');
        const text = document.getElementById('toast-text');
        text.textContent = message;
        toast.classList.remove('hidden');

        if (this.toastTimeout) clearTimeout(this.toastTimeout);
        this.toastTimeout = setTimeout(() => {
            toast.classList.add('hidden');
        }, durationMs);
    }

    async toggleAchievements() {
        const overlay = document.getElementById('achievements-overlay');
        if (!overlay.classList.contains('hidden')) {
            overlay.classList.add('hidden');
            return;
        }

        try {
            const achievements = await this.api.getAchievements();
            this.renderAchievements(achievements);
            overlay.classList.remove('hidden');
        } catch (error) {
            console.error('Failed to get achievements:', error);
        }
    }

    renderAchievements(achievements) {
        const grid = document.getElementById('achievements-grid');
        grid.innerHTML = '';

        for (const a of achievements) {
            const badge = document.createElement('div');
            badge.className = `achievement-badge ${a.unlocked ? 'unlocked' : 'locked'}`;
            badge.innerHTML = `
                <div class="badge-icon">${a.unlocked ? '+' : '?'}</div>
                <div class="badge-name">${a.unlocked ? a.name : '???'}</div>
                <div class="badge-desc">${a.unlocked ? a.description : '???'}</div>
            `;
            grid.appendChild(badge);
        }
    }

    async feedPet() {
        try {
            const result = await this.api.feedPet();
            this.handleActionResult(result);
            this.fetchStats();
        } catch (error) {
            console.error('Failed to feed pet:', error);
        }
    }

    async playWithPet() {
        try {
            const result = await this.api.playWithPet();
            this.handleActionResult(result);
            this.fetchStats();
        } catch (error) {
            console.error('Failed to play with pet:', error);
        }
    }

    async putToSleep() {
        try {
            const result = await this.api.putToSleep();
            this.handleActionResult(result);
            this.fetchStats();
        } catch (error) {
            console.error('Failed to put pet to sleep:', error);
        }
    }

    async revivePet() {
        try {
            const result = await this.api.revivePet();
            this.handleActionResult(result);
            this.fetchStats();
        } catch (error) {
            console.error('Failed to revive pet:', error);
        }
    }
}

// Start the app
new DotApp();

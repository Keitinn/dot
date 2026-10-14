const { invoke } = window.__TAURI__.core;

export class PetAPI {
    async getPetState() {
        return await invoke('get_pet_state');
    }

    async feedPet() {
        return await invoke('feed_pet');
    }

    async playWithPet() {
        return await invoke('play_with_pet');
    }

    async putToSleep() {
        return await invoke('put_to_sleep');
    }

    async revivePet() {
        return await invoke('revive_pet');
    }

    async getGameStats() {
        return await invoke('get_game_stats');
    }

    async getAchievements() {
        return await invoke('get_achievements');
    }

    async getSnacks() {
        return await invoke('get_snacks');
    }

    async startGame(game) {
        return await invoke('start_game', { game });
    }

    // One move of a game whose answer stays in Rust. input: { type: 'look' | 'guess' | ... }
    // -> { finished, moves, reward, view: { game, ... } }
    async gameStep(token, input) {
        return await invoke('game_step', { token, input });
    }

    async reportGameResult(token, game, outcome, moves = null) {
        return await invoke('report_game_result', { token, game, outcome, moves });
    }

    async buySnack(snackId) {
        return await invoke('buy_snack', { snackId });
    }

    async getLoadWarning() {
        return await invoke('get_load_warning');
    }

    // { enabled, mood, line: { id, text } | null, onboarding: 'prompt' | 'none' }
    async companionOpen() {
        return await invoke('companion_open');
    }

    // { enabled, mood }
    async refreshCompanionMood() {
        return await invoke('refresh_companion_mood');
    }

    // { master_on, quiet_start, quiet_end, quiet_until }
    async getCompanionSettings() {
        return await invoke('get_companion_settings');
    }

    // update: { master_on?, quiet_start?, quiet_end?, quiet?: 'quiet_1h' | 'quiet_today' | 'off' }
    async setCompanionSettings(update) {
        return await invoke('set_companion_settings', { update });
    }

    // answer: 'yes' | 'later'
    async companionOnboardingAnswer(answer) {
        return await invoke('companion_onboarding_answer', { answer });
    }

    // { enabled, path: string | null }
    async getSyncSettings() {
        return await invoke('get_sync_settings');
    }

    // update: { path?, enabled? }; rejects with a short message for an invalid folder
    async setSyncSettings({ path = null, enabled = null } = {}) {
        return await invoke('set_sync_settings', { path, enabled });
    }

    // { enabled, last_sync_at, unreadable, error, choice: [{ id, stage, age, mine }] | null }
    async getSyncStatus() {
        return await invoke('get_sync_status');
    }

    async syncNow() {
        return await invoke('sync_now');
    }

    async syncChoosePet(petId) {
        return await invoke('sync_choose_pet', { petId });
    }

    async quitApp() {
        return await invoke('quit_app');
    }
}

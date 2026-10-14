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

    async reportGameResult(token, game, outcome, moves = null) {
        return await invoke('report_game_result', { token, game, outcome, moves });
    }

    async buySnack(snackId) {
        return await invoke('buy_snack', { snackId });
    }

    async getLoadWarning() {
        return await invoke('get_load_warning');
    }

    async quitApp() {
        return await invoke('quit_app');
    }
}

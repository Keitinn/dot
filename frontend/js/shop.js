function el(tag, className, text) {
    const e = document.createElement(tag);
    if (className) e.className = className;
    if (text !== undefined) e.textContent = text;
    return e;
}

function effectText(s) {
    const parts = [];
    if (s.hunger) parts.push(`HUNGER +${s.hunger}`);
    if (s.happiness) parts.push(`HAPPY +${s.happiness}`);
    if (s.energy) parts.push(`ENERGY +${s.energy}`);
    if (s.health) parts.push(`HEALTH +${s.health}`);
    return parts.join('  ');
}

export class ShopUI {
    /**
     * @param api       PetAPI
     * @param getCoins  () => current coin count
     * @param onBought  (ActionResult) => void   (updates pet UI)
     */
    constructor(api, getCoins, onBought) {
        this.api = api;
        this.getCoins = getCoins;
        this.onBought = onBought;
        this.snacks = [];
        this.msgTimer = null;

        this.overlay = document.getElementById('shop-overlay');
        this.list = document.getElementById('shop-list');
        this.coins = document.getElementById('shop-coins');
        this.msg = document.getElementById('shop-msg');

        document.getElementById('shop-btn').addEventListener('click', (e) => {
            e.stopPropagation();
            this.overlay.classList.contains('hidden') ? this.open() : this.close();
        });
        document.getElementById('shop-close-btn').addEventListener('click', () => this.close());
    }

    async open() {
        try {
            if (this.snacks.length === 0) this.snacks = await this.api.getSnacks();
        } catch (e) {
            console.error('Failed to load snacks:', e);
            return;
        }
        this.overlay.classList.remove('hidden');
        this.say('');
        this.render();
    }

    close() {
        this.overlay.classList.add('hidden');
    }

    say(text) {
        this.msg.textContent = text;
        if (this.msgTimer) clearTimeout(this.msgTimer);
        if (text) this.msgTimer = setTimeout(() => (this.msg.textContent = ''), 2500);
    }

    /** Re-render prices/affordability. Safe to call whenever coins change. */
    render() {
        const coins = this.getCoins();
        this.coins.textContent = `$${coins}`;
        this.list.innerHTML = '';
        for (const s of this.snacks) {
            const row = el('div', 'shop-item');
            const info = el('div', 'shop-info');
            info.append(el('div', 'shop-name', s.name), el('div', 'shop-effect', effectText(s)));
            const buy = el('button', 'shop-buy', `$${s.price}`);
            buy.disabled = coins < s.price;
            buy.addEventListener('click', (ev) => {
                ev.stopPropagation();
                this.buy(s);
            });
            row.append(info, buy);
            this.list.appendChild(row);
        }
    }

    async buy(snack) {
        try {
            const result = await this.api.buySnack(snack.id);
            this.onBought(result);
            this.say(`ATE ${snack.name}!`);
        } catch (e) {
            // Tauri rejects with the Rust error string
            this.say(String(e).toUpperCase());
        }
        this.render();
    }
}

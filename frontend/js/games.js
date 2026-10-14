import {
    JANKEN_HANDS,
    judgeJanken,
    randomHand,
    createMemoryDeck,
    flipMemoryCard,
    createBlackjackDeck,
    handValue,
    isBlackjack,
    dealerPlay,
    judgeBlackjack,
} from './games-logic.js';

const HAND_ICON = { rock: '✊', paper: '✋', scissors: '✌' };
const HAND_LABEL = { rock: 'ROCK', paper: 'PAPER', scissors: 'SCISSORS' };
const RESULT_TEXT = {
    win: 'YOU WIN!',
    lose: 'YOU LOSE',
    draw: 'DRAW',
    push: 'PUSH',
    blackjack: 'BLACKJACK!',
};

const STORE_KEY = 'dot.games.v1';

function loadRecord() {
    try {
        return JSON.parse(localStorage.getItem(STORE_KEY)) || {};
    } catch {
        return {};
    }
}

function saveRecord(rec) {
    try {
        localStorage.setItem(STORE_KEY, JSON.stringify(rec));
    } catch {
        // storage unavailable: records just won't persist
    }
}

function el(tag, className, text) {
    const e = document.createElement(tag);
    if (className) e.className = className;
    if (text !== undefined) e.textContent = text;
    return e;
}

function button(label, onClick, className = 'game-btn') {
    const b = el('button', className, label);
    b.addEventListener('click', (ev) => {
        ev.stopPropagation();
        onClick(ev);
    });
    return b;
}

export class GamesUI {
    constructor() {
        this.overlay = document.getElementById('games-overlay');
        this.title = document.getElementById('games-title');
        this.body = document.getElementById('games-body');
        this.backBtn = document.getElementById('games-back-btn');
        this.record = loadRecord();
        this.timer = null;

        document.getElementById('games-btn').addEventListener('click', (e) => {
            e.stopPropagation();
            this.overlay.classList.contains('hidden') ? this.open() : this.close();
        });
        document.getElementById('games-close-btn').addEventListener('click', () => this.close());
        this.backBtn.addEventListener('click', () => this.showMenu());
    }

    open() {
        this.overlay.classList.remove('hidden');
        this.showMenu();
    }

    close() {
        this.clearTimer();
        this.overlay.classList.add('hidden');
    }

    clearTimer() {
        if (this.timer) {
            clearTimeout(this.timer);
            this.timer = null;
        }
    }

    setScreen(title, withBack) {
        this.clearTimer();
        this.title.textContent = title;
        this.backBtn.classList.toggle('hidden', !withBack);
        this.body.innerHTML = '';
    }

    bump(game, key) {
        const g = (this.record[game] = this.record[game] || {});
        g[key] = (g[key] || 0) + 1;
        saveRecord(this.record);
    }

    // ---------- Menu ----------
    showMenu() {
        this.setScreen('GAMES', false);
        const list = el('div', 'game-menu');
        list.appendChild(button('JANKEN', () => this.showJanken(), 'game-menu-btn'));
        list.appendChild(button('MEMORY', () => this.showMemory(), 'game-menu-btn'));
        list.appendChild(button('BLACKJACK', () => this.showBlackjack(), 'game-menu-btn'));
        this.body.appendChild(list);
    }

    // ---------- Janken ----------
    showJanken() {
        this.setScreen('JANKEN', true);
        const rec = (this.record.janken = this.record.janken || {});

        const board = el('div', 'game-board');
        const cpu = el('div', 'janken-hand', '?');
        const vs = el('div', 'game-note', 'CHOOSE YOUR HAND');
        const me = el('div', 'janken-hand', '?');
        const result = el('div', 'game-result', '');
        const score = el('div', 'game-note', '');
        const updateScore = () => {
            score.textContent = `W ${rec.win || 0}  L ${rec.lose || 0}  D ${rec.draw || 0}`;
        };
        updateScore();

        const choices = el('div', 'game-row');
        for (const hand of JANKEN_HANDS) {
            choices.appendChild(
                button(`${HAND_ICON[hand]}\n${HAND_LABEL[hand]}`, () => {
                    const cpuHand = randomHand();
                    const outcome = judgeJanken(hand, cpuHand);
                    me.textContent = HAND_ICON[hand];
                    cpu.textContent = HAND_ICON[cpuHand];
                    vs.textContent = `${HAND_LABEL[hand]} vs ${HAND_LABEL[cpuHand]}`;
                    result.textContent = RESULT_TEXT[outcome];
                    this.bump('janken', outcome);
                    updateScore();
                }, 'game-btn janken-choice')
            );
        }

        board.append(el('div', 'game-note', 'CPU'), cpu, vs, me, el('div', 'game-note', 'YOU'), result, choices, score);
        this.body.appendChild(board);
    }

    // ---------- Memory ----------
    showMemory() {
        this.setScreen('MEMORY', true);
        const rec = (this.record.memory = this.record.memory || {});
        let cards = createMemoryDeck();
        let moves = 0;
        let busy = false;

        const info = el('div', 'game-note', '');
        const result = el('div', 'game-result', '');
        const grid = el('div', 'memory-grid');
        const renderInfo = () => {
            info.textContent = `MOVES ${moves}  BEST ${rec.best ?? '-'}`;
        };

        const render = () => {
            grid.innerHTML = '';
            for (const c of cards) {
                const open = c.faceUp || c.matched;
                const cell = button(open ? c.suit : '?', () => onFlip(c.id), `memory-card${open ? ' open' : ''}${c.matched ? ' matched' : ''}`);
                cell.disabled = open || busy;
                grid.appendChild(cell);
            }
        };

        const onFlip = (id) => {
            if (busy) return;
            const r = flipMemoryCard(cards, id);
            if (!r.ok) return;
            if (r.pending) {
                moves++;
                busy = true;
                render();
                renderInfo();
                this.timer = setTimeout(() => {
                    for (const c of cards) if (r.pending.includes(c.id)) c.faceUp = false;
                    busy = false;
                    render();
                }, 700);
                return;
            }
            if (r.matched) moves++;
            if (r.done) {
                result.textContent = `CLEAR! ${moves} MOVES`;
                if (rec.best === undefined || moves < rec.best) {
                    rec.best = moves;
                    saveRecord(this.record);
                }
            }
            render();
            renderInfo();
        };

        const again = button('NEW GAME', () => this.showMemory());
        this.body.append(el('div', 'game-note', 'FIND THE 4 SUIT PAIRS'), grid, info, result, again);
        render();
        renderInfo();
    }

    // ---------- Blackjack ----------
    showBlackjack() {
        this.setScreen('BLACKJACK', true);
        const rec = (this.record.blackjack = this.record.blackjack || {});
        let deck;
        let player;
        let dealer;
        let over;

        const dealerRow = el('div', 'bj-hand');
        const playerRow = el('div', 'bj-hand');
        const dealerTotal = el('div', 'game-note', '');
        const playerTotal = el('div', 'game-note', '');
        const result = el('div', 'game-result', '');
        const score = el('div', 'game-note', '');
        const controls = el('div', 'game-row');

        const cardEl = (c, hidden) => el('span', `bj-card${hidden ? ' hidden-card' : ''}`, hidden ? '?' : `${c.rank}${c.suit}`);

        const renderScore = () => {
            score.textContent = `W ${(rec.win || 0) + (rec.blackjack || 0)}  L ${rec.lose || 0}  P ${rec.push || 0}`;
        };

        const render = () => {
            dealerRow.innerHTML = '';
            playerRow.innerHTML = '';
            dealer.forEach((c, i) => dealerRow.appendChild(cardEl(c, !over && i === 1)));
            player.forEach((c) => playerRow.appendChild(cardEl(c, false)));
            dealerTotal.textContent = over ? `DEALER ${handValue(dealer)}` : 'DEALER';
            playerTotal.textContent = `YOU ${handValue(player)}`;
            controls.innerHTML = '';
            if (over) {
                controls.appendChild(button('DEAL', deal));
            } else {
                controls.appendChild(button('HIT', hit));
                controls.appendChild(button('STAND', stand));
            }
            renderScore();
        };

        const finish = () => {
            over = true;
            const outcome = judgeBlackjack(player, dealer);
            result.textContent = RESULT_TEXT[outcome] + (handValue(player) > 21 ? ' (BUST)' : '');
            this.bump('blackjack', outcome);
            render();
        };

        const deal = () => {
            deck = createBlackjackDeck();
            player = [deck.pop(), deck.pop()];
            dealer = [deck.pop(), deck.pop()];
            over = false;
            result.textContent = '';
            if (isBlackjack(player) || isBlackjack(dealer)) {
                finish();
            } else {
                render();
            }
        };

        const hit = () => {
            if (over) return;
            player.push(deck.pop());
            if (handValue(player) >= 21) {
                if (handValue(player) === 21) dealerPlay(dealer, deck);
                finish();
            } else {
                render();
            }
        };

        const stand = () => {
            if (over) return;
            dealerPlay(dealer, deck);
            finish();
        };

        this.body.append(dealerTotal, dealerRow, playerRow, playerTotal, result, controls, score);
        deal();
    }
}

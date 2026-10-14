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
    minPlayMs,
    rewardText,
    settleText,
    canGuess,
    typeDigit,
    hitBlowRowText,
    hitBlowSlots,
    cardText,
    turnText,
    highLowEndText,
} from './games-logic.js';

const HAND_ICON = { rock: '✊', paper: '✋', scissors: '✌️' };
const HAND_LABEL = { rock: 'ROCK', paper: 'PAPER', scissors: 'SCISSORS' };
const RESULT_TEXT = {
    win: 'YOU WIN!',
    lose: 'YOU LOSE',
    draw: 'DRAW',
    push: 'PUSH',
    blackjack: 'BLACKJACK!',
};

const STORE_KEY = 'dot.games.v1';
// The session clock starts on the backend slightly after Date.now() here.
const REPORT_MARGIN_MS = 300;

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
    constructor(api, onReward) {
        this.api = api;
        this.onReward = onReward || (() => {});
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
        this.screenId = (this.screenId || 0) + 1;
        this.title.textContent = title;
        this.backBtn.classList.toggle('hidden', !withBack);
        this.body.innerHTML = '';
    }

    // Opens a server-side session; its token is needed to earn coins for this game.
    begin(game) {
        const token = this.api.startGame(game).catch((e) => {
            console.error('Failed to start game:', e);
            return null;
        });
        return { token, startedAt: Date.now() };
    }

    // Resolves to the RewardResult, or null when nothing was paid.
    async reward(session, game, outcome, moves = null) {
        try {
            const token = await session.token;
            if (!token) return null;
            // The backend rejects reports that come in before the minimum play time.
            const wait = minPlayMs(game, moves) + REPORT_MARGIN_MS - (Date.now() - session.startedAt);
            if (wait > 0) await new Promise((resolve) => setTimeout(resolve, wait));
            const r = await this.api.reportGameResult(token, game, outcome, moves);
            this.onReward(r);
            return r;
        } catch (e) {
            console.error('Failed to report game result:', e);
            return null;
        }
    }

    // ---------- Games whose answer stays in Rust ----------
    // Every move goes through game_step and Rust decides the coins, so these
    // reports carry no outcome.

    // True while the screen that asked is still shown.
    liveCheck() {
        const id = this.screenId;
        return () => id === this.screenId && !this.overlay.classList.contains('hidden');
    }

    // Resolves after `ms`; never resolves if the screen changes first.
    wait(ms) {
        return new Promise((resolve) => {
            this.timer = setTimeout(() => {
                this.timer = null;
                resolve();
            }, ms);
        });
    }

    // Resolves to { token, startedAt, result } with the first look, or null.
    async play(game) {
        const startedAt = Date.now();
        try {
            const token = await this.api.startGame(game);
            const result = await this.api.gameStep(token, { type: 'look' });
            return { token, startedAt, result };
        } catch (e) {
            console.error('Failed to start game:', e);
            return null;
        }
    }

    // Resolves to the StepResult, or null when Rust refused the move.
    async move(play, input) {
        try {
            return await this.api.gameStep(play.token, input);
        } catch (e) {
            console.error('Game move refused:', e);
            return null;
        }
    }

    // Reports a finished game. Resolves to the RewardResult, or null when refused.
    async settle(play, game, result) {
        try {
            const wait = minPlayMs(game, result.moves) + REPORT_MARGIN_MS - (Date.now() - play.startedAt);
            if (wait > 0) await new Promise((resolve) => setTimeout(resolve, wait));
            const r = await this.api.reportGameResult(play.token, game, 'done', null);
            this.onReward(r);
            return r;
        } catch (e) {
            console.error('Failed to report game result:', e);
            return null;
        }
    }

    // Shown when a game could not start; NEW GAME tries again.
    showStartFailed(again) {
        this.body.append(el('div', 'game-result', 'TRY AGAIN'), button('NEW GAME', again));
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
        list.appendChild(button('HIT & BLOW', () => this.showHitBlow(), 'game-menu-btn'));
        list.appendChild(button('HIGH & LOW', () => this.showHighLow(), 'game-menu-btn'));
        this.body.appendChild(list);
    }

    // ---------- HIT & BLOW ----------
    async showHitBlow() {
        this.setScreen('HIT & BLOW', true);
        const live = this.liveCheck();
        const rec = (this.record.hit_blow = this.record.hit_blow || {});
        const play = await this.play('hit_blow');
        if (!live()) return;
        if (!play) return this.showStartFailed(() => this.showHitBlow());

        let typed = [];
        let busy = false;
        const slots = el('div', 'game-result hb-slots', '');
        const history = el('div', 'hb-history');
        const info = el('div', 'game-note', '');
        const result = el('div', 'game-result', '');
        const keypad = el('div', 'hb-keypad');
        const controls = el('div', 'game-row');

        const render = (view) => {
            slots.textContent = view.answer ? view.answer.join(' ') : hitBlowSlots(typed);
            history.innerHTML = '';
            for (const row of view.guesses) history.appendChild(el('div', 'hb-row', hitBlowRowText(row)));
            info.textContent = `LEFT ${view.left}  BEST ${rec.best ?? '-'}`;
            for (const key of keypad.children) {
                key.disabled = busy || !!view.answer || typed.includes(Number(key.dataset.digit));
            }
            ok.disabled = busy || !!view.answer || !canGuess(typed);
            del.disabled = busy || !!view.answer || typed.length === 0;
        };

        let view = play.result.view;
        for (let d = 0; d <= 9; d++) {
            const key = button(String(d), () => {
                typed = typeDigit(typed, d);
                render(view);
            }, 'game-btn hb-key');
            key.dataset.digit = String(d);
            keypad.appendChild(key);
        }
        const del = button('DEL', () => {
            typed = typed.slice(0, -1);
            render(view);
        });
        const ok = button('OK', async () => {
            if (!canGuess(typed)) return;
            busy = true;
            render(view);
            const r = await this.move(play, { type: 'guess', digits: typed });
            if (!live()) return;
            busy = false;
            if (!r) {
                result.textContent = 'TRY AGAIN';
                render(view);
                return;
            }
            typed = [];
            view = r.view;
            result.textContent = '';
            render(view);
            if (r.finished) finish(r);
        });
        controls.append(del, ok);

        const finish = (r) => {
            const base = view.solved ? `CLEAR! ${view.guesses.length} GUESSES` : 'OUT OF GUESSES';
            result.textContent = base;
            if (view.solved && (rec.best === undefined || view.guesses.length < rec.best)) {
                rec.best = view.guesses.length;
                saveRecord(this.record);
            }
            controls.innerHTML = '';
            controls.appendChild(button('NEW GAME', () => this.showHitBlow()));
            this.settle(play, 'hit_blow', r).then((reward) => {
                if (live()) result.textContent = base + settleText(reward);
            });
        };

        this.body.append(el('div', 'game-note', '3 DIGITS, NO REPEATS. H=SPOT B=DIGIT'), slots, history, keypad, controls, info, result);
        render(view);
    }

    // ---------- HIGH & LOW ----------
    async showHighLow() {
        this.setScreen('HIGH & LOW', true);
        const live = this.liveCheck();
        const rec = (this.record.high_low = this.record.high_low || {});
        const play = await this.play('high_low');
        if (!live()) return;
        if (!play) return this.showStartFailed(() => this.showHighLow());

        let busy = false;
        const card = el('div', 'bj-card hl-card', '');
        const last = el('div', 'game-note', '');
        const info = el('div', 'game-note', '');
        const result = el('div', 'game-result', '');
        const controls = el('div', 'game-row');

        const render = (view) => {
            card.textContent = cardText(view.card);
            last.textContent = turnText(view.last);
            info.textContent = `STREAK ${view.streak}  STOP +$${view.banked}  BEST ${rec.best ?? '-'}`;
            for (const b of controls.children) b.disabled = busy || !!view.end;
        };

        const act = async (input) => {
            busy = true;
            render(view);
            const r = await this.move(play, input);
            if (!live()) return;
            busy = false;
            if (!r) {
                result.textContent = 'TRY AGAIN';
                render(view);
                return;
            }
            view = r.view;
            result.textContent = '';
            render(view);
            if (r.finished) finish(r);
        };

        const finish = (r) => {
            const base = highLowEndText(view.end);
            result.textContent = base;
            if (view.end !== 'lost' && (rec.best === undefined || view.streak > rec.best)) {
                rec.best = view.streak;
                saveRecord(this.record);
            }
            controls.innerHTML = '';
            controls.appendChild(button('NEW GAME', () => this.showHighLow()));
            this.settle(play, 'high_low', r).then((reward) => {
                if (live()) result.textContent = base + settleText(reward);
            });
        };

        let view = play.result.view;
        controls.append(
            button('HIGH', () => act({ type: 'high' })),
            button('LOW', () => act({ type: 'low' })),
            button('STOP', () => act({ type: 'stop' })),
        );
        this.body.append(el('div', 'game-note', 'IS THE NEXT CARD HIGHER OR LOWER?'), card, last, info, controls, result);
        render(view);
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

        // Each throw is one game: report it with the current session and open the next.
        // The hands stay locked until the report lands, so throws cannot outpace the
        // backend's minimum play time or pile up pending sessions.
        let session = this.begin('janken');
        const choices = el('div', 'game-row');
        const lock = (locked) => {
            for (const b of choices.children) b.disabled = locked;
        };
        for (const hand of JANKEN_HANDS) {
            choices.appendChild(
                button(`${HAND_ICON[hand]}\n${HAND_LABEL[hand]}`, async () => {
                    const played = session;
                    session = this.begin('janken');
                    lock(true);
                    const cpuHand = randomHand();
                    const outcome = judgeJanken(hand, cpuHand);
                    me.textContent = HAND_ICON[hand];
                    cpu.textContent = HAND_ICON[cpuHand];
                    vs.textContent = `${HAND_LABEL[hand]} vs ${HAND_LABEL[cpuHand]}`;
                    result.textContent = RESULT_TEXT[outcome];
                    this.bump('janken', outcome);
                    updateScore();
                    const r = await this.reward(played, 'janken', outcome);
                    result.textContent = RESULT_TEXT[outcome] + rewardText(r);
                    lock(false);
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
        const session = this.begin('memory');
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
                this.reward(session, 'memory', 'clear', moves).then((r) => {
                    result.textContent = `CLEAR! ${moves} MOVES${rewardText(r)}`;
                });
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
        let session;
        let hand = 0;

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
            const base = RESULT_TEXT[outcome] + (handValue(player) > 21 ? ' (BUST)' : '');
            result.textContent = base;
            this.bump('blackjack', outcome);
            render();
            const thisHand = hand;
            this.reward(session, 'blackjack', outcome).then((r) => {
                if (thisHand === hand) result.textContent = base + rewardText(r);
            });
        };

        const deal = () => {
            session = this.begin('blackjack');
            hand++;
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

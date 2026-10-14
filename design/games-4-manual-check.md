# 追加ミニゲーム 4 種: 実機の手動確認チェックリスト

対象: HIT & BLOW / HIGH & LOW / DOT SAYS / WHERE'S DOT?(`feat/games-4`)。
`npm run dev` で起動し、メニューバーの dot を開いて `G` から確認する。

## 共通
- [ ] `GAMES` メニューが 2 列で 7 つ並び、280×380 に収まる(スクロールなし)。既存 3 ゲームも従来どおり動く
- [ ] 各ゲームの `<` でメニューに戻れ、遊び途中で戻っても次の画面で古い表示が出ない(タイマーが残らない)
- [ ] ゲーム途中で `X` で閉じて開き直すと、メニューから始まる
- [ ] 終了時に `+$N` が出て、右上のコインが同じだけ増える。0 枚のときは何も付かない
- [ ] 日次上限: その日 28 枚稼いだ状態で 8 枚の報酬 → `+$2 DAILY LIMIT`、上限到達後は ` DAILY LIMIT` だけ
- [ ] 通信・検証エラー時は結果の行に `TRY AGAIN` が出る(無言にならない)
- [ ] 点滅・高速な動きがない。音が出ない

## HIT & BLOW
- [ ] 数字キー 0–9、`DEL`、`OK`。同じ数字は 2 回押せない(押したキーは灰色)、3 桁未満では `OK` が押せない
- [ ] 履歴が `012 1H 2B` の形で 2 列に並び、8 手でも画面に収まる
- [ ] 正解で `CLEAR! N GUESSES`、8 手外すと `OUT OF GUESSES` と答えが表示される
- [ ] 1 手目で正解 → +$8、8 手目で正解 → +$1、失敗 → 0

## HIGH & LOW
- [ ] 大きめのカード(例 `Q♦`)、直前の判定(`HIGH 9♥ RIGHT` / `TIE` / `MISS`)、`STREAK n  STOP +$n`
- [ ] 同じ数字は `TIE` で連勝数が変わらない。外すと `MISS` で 0 枚
- [ ] `STOP` でその時点の連勝数ぶん(最大 5)もらえる。連勝 5 で `TOP STREAK!` で自動終了

## DOT SAYS
- [ ] 4 つのパッドが 1 つずつ黒くなって戻る(約 0.8 秒間隔)。同じパッドが続いても 2 回と分かる
- [ ] `WATCH` の間は押せず、`YOUR TURN` で押せる。1 つ間違えると即終了 `MISS - n ROUNDS`
- [ ] 8 ラウンドクリアで `ALL 8 ROUNDS!` と +$5。3 ラウンド → +$1、5 → +$3
- [ ] macOS の「視差効果を減らす」(prefers-reduced-motion) ON: パッドは光らず、順番が `1 4 2` の文字で一定時間出て消える

## WHERE'S DOT?
- [ ] 最初に 1 つのカップが白くなり `●` が見え、黒く伏せてから 4〜6 回ゆっくり(約 0.7 秒ずつ)入れ替わる
- [ ] 入れ替え後に `WHERE'S DOT?` で選べる。選ぶと dot の位置が開き、`FOUND DOT!` / `NOT THERE`
- [ ] 3 ラウンドで終了。正解 1 回 1 枚、3 回全部で +$4
- [ ] prefers-reduced-motion ON: カップは動かず番号 `1 2 3` が付き、`SWAP 1/5: 1 <> 3` を `NEXT` で 1 手ずつ進める。最後に番号で選ぶ

## 不正の試行(devtools)
`npm run dev` で開いたウィンドウで右クリック → 検証(または Web Inspector)→ Console。
`const { invoke } = window.__TAURI__.core;` を最初に実行する。

- [ ] 申告を無視する: `t = await invoke('start_game', { game: 'hit_blow' })` → 10 秒待って `await invoke('report_game_result', { token: t, game: 'hit_blow', outcome: 'clear', moves: 1 })` → `hit_blow is not over yet` で拒否され、コインは増えない
- [ ] 答えが漏れない: `t = await invoke('start_game', { game: 'hit_blow' })`; `await invoke('game_step', { token: t, input: { type: 'look' } })` → `view.answer` が `null`
- [ ] 不正な入力: `await invoke('game_step', { token: t, input: { type: 'guess', digits: [1, 1, 2] } })` → `digits must differ`。続けて `look` で `left` が 8 のまま
- [ ] 他のゲームの手: `await invoke('game_step', { token: t, input: { type: 'high' } })` → `not a move in this game`
- [ ] DOT SAYS で先の順番が見えない: `t = await invoke('start_game', { game: 'dot_says' })`; `look` → `view.show` が 1 つだけ
- [ ] WHERE'S DOT? で最終位置が来ない: `t = await invoke('start_game', { game: 'wheres_dot' })`; `look` → `view` に `start`・`swaps` はあるが、終わった位置のキーはない(`picks` は空)
- [ ] 速すぎる報告: HIGH & LOW を `look` → `{ type: 'stop' }` → 即 `report_game_result` → `game finished too quickly`。同じトークンでもう一度報告 → `unknown or used game token`
- [ ] 既存ゲームに `game_step`: `t = await invoke('start_game', { game: 'janken' })`; `game_step` → `janken has no steps`

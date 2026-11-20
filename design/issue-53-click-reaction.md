# Issue #53: ポップオーバー内の dot をクリックしたときの反応

> **Status**: Draft(実装前の設計。レビュー待ち)
> **対象 Issue**: Keitinn/dot #53
> **コード基準**: `feat/issue-53` の分岐元 `27f7b0e`
> **関連**: `design/gdd/companion.md`(R2 Mood / R3 台詞 / 0.3 ピラーとアンチピラー / UI Requirements)

## 0. 要約

ポップオーバーのキャンバス上の dot をクリック(またはキーボードで押下)すると、dot が **短い動き・表情の変化** と、ときどき **1 行の吹き出し** で反応する。反応の中身は、成長段階・生死・活動(睡眠など)・空腹など・時間帯・大人期の Mood で変わる。連打には「間隔を空ける」と「目が回る反応のあと休む」で応える。

推奨する方式は **フロントエンドだけで完結する軽量な仕組み(案 B)** である。吹き出しの DOM・live region・行の分割(`splitLine`)は既存のコンパニオンのものを使い回す。大人期の Mood は、バックエンドが決めた値をそのまま読む。台詞の選択は新しい純関数モジュール `frontend/js/tap-logic.js` に置き、`node --test` で検証する。**ステータス・コイン・Bond・セーブは一切変えない。**

---

## 1. 現状のコード調査

### 1.1 クリックを受ける要素

| 項目 | 場所 | 内容 |
|---|---|---|
| キャンバス | `frontend/index.html:49` | `<canvas id="pet-canvas" width="128" height="128">`。role・tabindex・aria-label はない |
| キャンバスの親 | `frontend/index.html:48-52` | `#pet-container` の中に、キャンバス・`#companion-bubble`・`#sync-choice` が並ぶ |
| クリックハンドラ | なし | `pet-canvas` を参照するのは `frontend/js/main.js:15`(`new PetRenderer('pet-canvas')`)だけで、クリックイベントは登録されていない |
| スタイル | `frontend/styles/main.css:123-129` | `#pet-canvas` は 128px 四方で黒枠。`cursor` の指定はない |
| 吹き出しの層 | `frontend/styles/main.css:1005-1014` | `#companion-bubble` はキャンバス上部に絶対配置(`top: calc(50% - 60px)`)、`pointer-events: none`。中の `.companion-bubble` だけが `pointer-events: auto`(`main.css:1030`)なので、吹き出しの外側のキャンバスはクリックを受けられる |
| 同期の選択 | `frontend/styles/main.css:964-973` | `#sync-choice` はキャンバス下部に重なる(`z-index: 11`、外枠は `pointer-events: none`) |
| キャンバスを覆う画面 | `main.css:237-247`(death-overlay, z 100)、`main.css:316-326`(achievements, z 90)、`main.css:427-437`(games, z 95)、shop-overlay(`main.css:735-`) | これらが開いている間、クリックはキャンバスに届かない |
| 大人の死(Farewell) | `main.css:288-301`、`main.js:163` | `#app.farewell` ではキャンバスは見えたままで、`#companion-bubble` は `display: none`。death-overlay は static になり、キャンバスはクリックできる状態で残る |
| ドキュメント全体のクリック | `main.js:72`、`companion.js:247` | `document` の click で `*` メニューと COMPANION 設定を閉じている。キャンバスのクリックでもこれらが閉じる(問題なし。むしろ望ましい) |

### 1.2 描画(スプライトと表情)

| 項目 | 場所 | 内容 |
|---|---|---|
| 描画ループ | `frontend/js/renderer.js:38-75` | rAF ループ。`animTime`(秒)を進め、`main.js:98-102` のコールバックで毎フレーム `render(petState)` を呼ぶ |
| 表示中だけ動く | `main.js:84-110` | `shouldRun`(`visibility.js`)で、非表示中はループとポーリングを止める |
| 段階ごとの描画 | `renderer.js:110-132` | `Egg / Baby / Child(Teen) / Adult` で分岐する。不明な段階は Egg として描く |
| Egg | `renderer.js:134-166` | 揺れる(`wobble`, `tilt`)。目はない |
| Baby | `renderer.js:168-208` | 小さく跳ねる(`hop` は最大 3px、`squash` あり)。目は固定の 2 点(`renderer.js:198-205`) |
| Child | `renderer.js:210-252` | 左右に揺れる(`wiggle` 2px)。目は固定(`renderer.js:242-249`) |
| Adult | `renderer.js:254-308` | 呼吸と揺れ。`stillAdult` なら止める(`renderer.js:259-260`)。`this.face` があれば Mood の目を描き(`renderer.js:290-296`)、なければ従来の目を描く |
| 大人の目 | `renderer.js:311-321`、`companion-logic.js:53-68` | 6×6 の目を 3×3 セルの `EYES` 表で描く。形は `calm / concerned / sleepy / missed_you / cheerful / blink / closed` |
| 表情の切り替え | `renderer.js:328-336` | `setFace(mood, { reducedMotion, still })`。新しい表情に変わると 1 回瞬きする(`BLINK_MS = 160`、`renderer.js:4`)。動きを減らす設定では瞬きせず、体も止める |
| 表情の同期 | `companion.js:79-86`、`main.js:156` | `updateUI()` のたびに `syncFace()` が呼ばれる。5 秒ごとのポーリング(`main.js:103`)でも呼ばれる |
| 睡眠の絵 | なし | `sprites.js:1-76` は使われていない定義で、睡眠・食事・遊びの専用の絵はまだない。睡眠中だと絵でわかるのは、大人の `sleepy` の目(Mood が sleepy のとき)だけ |

### 1.3 既存の吹き出しと台詞選択

| 項目 | 場所 | 内容 |
|---|---|---|
| 吹き出しの表示 | `companion.js:134-184` | `showBubble(content)`。`content.rows` を `<br>` で並べ、`prompt` なら YES/LATER ボタンも出す。クリックで消え(`companion.js:158`)、ホバー中・フォーカス中は消えない(`companion.js:159-172`) |
| 自動フェード | `companion.js:186-209`、`companion-logic.js:4,43-51` | `BUBBLE_TTL_MS = 6000`。動きを減らす設定ではフェードせず即座に消す(`companion.js:201-204`)。VoiceOver が有効かは検出できないので、通常どおり消える(`companion.js:190`) |
| 消去 | `companion.js:211-218` | `dismiss()`。blur・非表示・Escape で呼ばれる(`companion.js:54-62`) |
| live region | `index.html:50` | `#companion-bubble` に `role="status" aria-live="polite"` が付いている。吹き出しはここに追加される(`companion.js:47,177`) |
| 行の分割 | `companion-logic.js:5-31` | `LINE_MAX_CHARS = 24`、`ONE_ROW_MAX = 14`。最大 2 行 |
| 吹き出しの中身 | `companion-logic.js:33-41` | `bubbleContent(result)` が `{ kind: 'line' | 'prompt', rows }` を返す |
| 開いたときの呼び出し | `companion.js:88-110`、`companion-logic.js:117-123` | window が focus されるたびに `companion_open` を呼ぶ(2 秒以内の再 focus は同じ open とみなす)。結果が来たら `dismiss()` してから台詞を出す(`companion.js:106,109`) |
| 台詞データ | `backend/src/companion/lines.json`(68 行) | `{ id, category, text, min_tier, part?, need?, mood? }`。Rust に `include_str!` で埋め込む(`lines.rs:97-103`) |
| 台詞選択 | `backend/src/companion/lines.rs:129-150` | R3 のカテゴリを優先順に評価する。`time_of_day` と `mood` は確率 `LINE_SPEAK_P = 0.5`(`consts.rs:67`)で話す。直近 20 件(`LINE_HISTORY`)の重複を避け、カテゴリごとに 24 時間のクールダウンがある(`lines.rs:174-189`) |
| 時間帯 | `lines.rs:49-72` | `Part::of_hour`: 05–10 朝 / 11–16 昼 / 17–21 夕 / 22–04 夜 |
| 開いたときの処理 | `backend/src/companion/open.rs:46-97` | `is_active(pet)`(生きている大人)でなければ `enabled: false`。muted(マスター OFF または QUIET 中)なら台詞なし(`open.rs:71-76`) |
| コマンド | `backend/src/commands.rs:536-571`、`604-616` | `companion_open` は Bond の来訪を数え、`companion.json` を保存し、同期が ON なら同期も走らせる。`refresh_companion_mood` は来訪を数えない |
| 台詞データの検査 | `lines.rs:645-790` | 文字数・2 行以内・文字種(大文字と `.,?'-` だけで感嘆符なし)・ID の一意性・禁止語(`lines.rs:649-656`: `LONELY` `SAD` `PET ME` `PLEASE` `WORK` など) |

### 1.4 Mood と状態

| 項目 | 場所 | 内容 |
|---|---|---|
| Mood の決定 | `backend/src/companion/mood.rs:107-151` | R2 の表を上から評価する: concerned(needs の最小値 < 20 か health < 30、解除は 30)→ sleepy(睡眠中 / energy < 35 / 23–06 時)→ missed_you(開いた瞬間だけ)→ calm(needs の最小値 < 60)→ cheerful |
| フロントの Mood | `companion-logic.js:11,70-75`、`companion.js:73-76` | `faceMood(result, pet)`: 生きている大人で companion が有効なときだけ Mood を返し、それ以外は null。`CompanionUI.mood()` で読める |
| 状態の文字列 | `backend/src/pet/state.rs:243-274` | `status()`: `sleeping (Xh Ym left)` / `eating` / `playing` / Idle なら `hungry`(hunger < 20)/ `tired`(energy < 20)/ `sad`(happiness < 20)/ `content` |
| 活動 | `state.rs:5-11,21-22` | `Idle / Sleeping / Eating / Playing`。食事と遊びは 30 秒 |
| 情報行 | `companion-logic.js:81-86` | 大人で Idle のときは、status の代わりに Mood 名を出す |
| 死の画面 | `companion-logic.js:88-115` | 大人は FAREWELL(`eyes: 'closed'`)、それ以外は GAME OVER |
| muted の判定 | `backend/src/companion/settings.rs`(`is_muted`)、`commands.rs:618-622` | `get_companion_settings` が `{ master_on, quiet_start, quiet_end, quiet_until }` を返す。フロントは設定パネルを開いたときにしか取得していない(`companion.js:270-284`) |

### 1.5 テスト

| ファイル | 内容 |
|---|---|
| `tests/companion-logic.test.mjs` | `splitLine` / `bubbleContent` / `shouldAutoDismiss` / `eyeGrid` / `faceMood` / `shouldBlink` / `statusText` / `deathView` / `shouldCallOpen` / `parseHour` |
| `tests/games-logic.test.mjs`, `sync-logic.test.mjs`, `visibility.test.mjs`, `window-config.test.mjs`, `signing-config.test.mjs` | 他の機能 |
| 実行方法 | `package.json` の `"test": "node --test tests/*.test.mjs"`。DOM を使わない純関数だけをテストする方針(`companion-logic.js:1-2` のコメント) |
| Rust | `backend/src/companion/{lines,open,mood}.rs` の `#[cfg(test)]`。台詞データの検査もここにある |

---

## 2. 受け入れ条件

QA が合否を判定できる形で書く。「反応する」は「キャンバス上に動きか表情の変化が 1 回以上描かれる」という意味とする。

**基本**
- AC1. 生きている Baby / Child / Adult の dot をクリックすると、200ms 以内に反応(動きまたは表情)が始まり、`TAP_REACT_MS`(600ms)以内に元に戻る。
- AC2. 台詞を伴う反応では、既存と同じ見た目の吹き出しに 1 行(最大 2 行・24 文字)が出る。吹き出しは既存の規則どおり、クリックで消え、ホバー中は残り、`BUBBLE_TTL_MS` 後にフェードする。
- AC3. クリックしても `get_pet_state` と `get_game_stats` の値(hunger / happiness / energy / health / coins / streak / 実績)は変わらない。`companion.json` と `pet.json` は書き込まれない(ファイルの更新時刻が変わらない)。
- AC4. クリックは、Bond の来訪(`today.g`)にも世話の回数にも数えない。

**状態別**
- AC5. 死亡中(GAME OVER・FAREWELL)はクリックしても何も起きない(動き・表情・吹き出し・読み上げのいずれもない)。
- AC6. 睡眠中にクリックしても dot は起きない(`activity` は `Sleeping` のまま)。動きはなく、`ZZZ...` 系の台詞だけを出す。
- AC7. Egg は台詞を出さず、揺れが一度だけ少し大きくなる。
- AC8. 状態ごとの台詞は §4.4 の表のとおりに選ばれる(純関数のテストで確認する)。
- AC9. 大人期で companion が有効なときは、画面に出ている Mood(`CompanionUI.mood()`)に合った台詞だけが選ばれる。台詞を選んでも Mood は変わらない。

**連打**
- AC10. 前に受け付けたクリックから `TAP_MIN_GAP_MS`(700ms)未満のクリックは無視する(反応も台詞もない)。
- AC11. `TAP_STREAK_WINDOW_MS`(5 秒)以内に `TAP_STREAK_DIZZY`(5)回受け付けると、目が回る反応(`DIZZY...` 系の台詞)を 1 回出す。その後の `TAP_REST_MS`(10 秒)は台詞を出さず、瞬きだけを返す。
- AC12. 直近 `TAP_HISTORY`(4)件と同じ台詞 ID は出ない(その状態の候補が 5 件以上ある場合)。

**他の吹き出しとの関係**
- AC13. `STAY WITH YOU?` の確認、または開いたときの台詞が出ている間にクリックしても、その吹き出しは置き換わらない(動きだけを返す)。
- AC14. COMPANION のマスター OFF、または QUIET 1H / QUIET TODAY が有効な間は、どの段階でも台詞を出さず、動きだけを返す。静寂時間(22–08)は台詞を止めない(GDD の `quiet_hours` と同じ扱い)。

**アクセシビリティ**
- AC15. キャンバスに Tab でフォーカスでき、Enter と Space でクリックと同じ反応をする。フォーカスリングが見える。
- AC16. 台詞は `#companion-bubble`(`role="status"`, `aria-live="polite"`)に入り、VoiceOver で読み上げられる。台詞のない反応は読み上げない。
- AC17. `prefers-reduced-motion: reduce` のときは、位置の動き(跳ね・大きな揺れ)と瞬きを起こさない。表情の差し替え(静止画)と台詞は出す。

**性能**
- AC18. クリック 1 回で IPC は発生しない(DevTools の invoke ログで確認する)。描画ループのほかにタイマーを増やさない(反応の終わりは、描画のたびに時刻を比べて判定する)。

---

## 3. 方式の案と比較

### 案 A: 既存コンパニオン基盤(バックエンド)を流用する

`lines.json` に `tap_*` カテゴリを足し、新しいコマンド `companion_tap` で `lines::select` を通して台詞を返す。

- 良い点: 台詞データが 1 か所にまとまり、Rust の既存の検査(禁止語・文字数)がそのまま効く。大人期の Mood と同じ場所で選ぶので、整合が取りやすい。
- 悪い点:
  - Companion は「生きている大人」専用(`open.rs:55-57`、GDD R1)。Baby / Child に広げると、R1 と `is_active` の意味が変わり、既存テスト(`a_pet_that_is_not_an_adult_is_disabled_and_untouched` など)の前提が崩れる。
  - 選択の仕組み(カテゴリごとの 24 時間クールダウン、`LineHistory` の永続化)は「開いたときの 1 行」のためのもので、数秒単位のタップに合わない。タップのたびに `companion.json` を保存し、同期が ON なら `observe_companion` まで走る(`commands.rs:553-564`)のは過剰。
  - クリックのたびに IPC とロック(`pet` → `companion`)が入る。
  - Rust とフロントの両方を変える必要があり、PR が大きくなる。

### 案 B: フロントエンドだけの軽量な仕組み(推奨)

`frontend/js/tap-logic.js`(純関数と定数)と `frontend/js/tap-lines.js`(台詞データ)を新しく作り、`frontend/js/tap.js`(DOM とイベント)で組み立てる。吹き出しの表示は、`CompanionUI` の `showBubble` を公開メソッド `say(rows)` として使い回す。

- 良い点:
  - 永続化も IPC もないので、AC3・AC4・AC18 を構造的に満たす。
  - 全段階に対応しても、Companion の R1 を変えずに済む。
  - 吹き出し・live region・フェード・Escape や blur での消去は、既存のものがそのまま効く。
  - 純関数なので、`node --test` で状態表・連打・乱数を決定的にテストできる。
- 悪い点:
  - 台詞データが Rust(`lines.json`)と JS(`tap-lines.js`)の 2 か所に分かれる。→ 禁止語・文字種・文字数の検査を node のテストにも置き、規則を揃える(§5)。
  - 時間帯の区切り(`Part::of_hour`)を JS に複製することになる。→ 境界値のテストを Rust と同じ値で書く。
  - 連打の履歴はメモリ上にしかなく、ポップオーバーを閉じると消える(仕様として許容する。§4.3)。

### 案 C: 動きと表情だけ(台詞なし)

- 良い点: 最小限で済む。データもテストもほとんど要らない。
- 悪い点: Issue の「おしゃべり」に応えられない。状態で反応を変える余地も小さい。

### 比較

| 観点 | 案 A | 案 B | 案 C |
|---|---|---|---|
| Issue の要望(台詞・状態別・連打) | ○ | ○ | △(台詞なし) |
| GDD R1(大人のみ)を変えない | ×(全段階に広げると変わる) | ○ | ○ |
| ステータス・セーブを変えない | △(保存が走る) | ○ | ○ |
| 変更範囲 | Rust + JS | JS のみ | JS のみ |
| テストのしやすさ | ○(Rust) | ○(node) | ○ |
| 台詞データの一元化 | ○ | △(2 か所) | — |
| 軽さ(IPC・I/O) | △ | ○ | ○ |

**推奨: 案 B。** タップは「ユーザーが触ったことへのその場の応答」であり、関係の記録(Bond・台詞の履歴)には入れない。この性格の違いを、そのまま構造に反映できる。台詞データが 2 か所になる弱点は、同じ規則の検査を node 側にも置いて抑える。

---

## 4. 詳細設計(案 B)

### 4.1 モジュール構成

| ファイル | 種類 | 役割 |
|---|---|---|
| `frontend/js/tap-lines.js`(新規) | データ | `export const TAP_LINES = [...]`。JSON の import 属性には頼らず、ES モジュールの配列にする(WebView と node の両方でそのまま読める) |
| `frontend/js/tap-logic.js`(新規) | 純関数 | 定数、`tapSituation`、`acceptTap`、`pickTapLine`、`rememberTap`、`tapMotion`、`partOfHour`、`isMuted`。DOM は使わない |
| `frontend/js/tap.js`(新規) | DOM | `TapUI`: キャンバスのクリックとキー入力を受け、純関数を呼び、結果を `renderer.react()` と `companion.say()` に渡す |
| `frontend/js/companion.js` | 変更 | `say(rows)`(タップ用の吹き出し)と `isShowingOwnBubble()` を足す。muted を把握する(`companion_open` の後に設定を 1 回取得する) |
| `frontend/js/renderer.js` | 変更 | `react({ motion, face, until })` を足し、各段階の描画に反応のオフセットと表情を重ねる |
| `frontend/js/main.js` | 変更 | `TapUI` を生成する |
| `frontend/index.html` | 変更 | キャンバスに `tabindex="0" role="button" aria-label="dot"` を付ける |
| `frontend/styles/main.css` | 変更 | `#pet-canvas { cursor: pointer; }` と `:focus-visible` の枠 |
| `tests/tap-logic.test.mjs`(新規) | テスト | §5 |

### 4.2 台詞データの形式

```js
// frontend/js/tap-lines.js
export const TAP_LINES = [
    { id: 'tap_baby_1', situation: 'baby', text: 'BA?' },
    { id: 'tap_adult_cheerful_1', situation: 'adult', mood: 'cheerful', text: 'HEHE. HI.' },
    { id: 'tap_adult_night_1', situation: 'adult', part: 'night', text: 'STILL UP?' },
    // ...
];
```

| フィールド | 必須 | 値 | 意味 |
|---|---|---|---|
| `id` | ○ | `tap_` で始まり、小文字・数字・`_` だけ | 一意。接頭辞があるので `lines.json` の ID とも重ならない |
| `situation` | ○ | §4.4 の situation 名、または `dizzy` | どの状況の台詞か |
| `text` | ○ | 大文字の英語と `.,?'-` だけで 24 文字以内。2 行にしたい場合も 1 行で書き、`splitLine` に任せる | GDD のトーン規約と `lines.rs:687-699` の検査に合わせる。感嘆符は使わない |
| `mood` | 任意 | `MOODS` のいずれか | `situation: 'adult'` のとき、その Mood でだけ使う |
| `part` | 任意 | `morning` / `day` / `evening` / `night` | その時間帯にだけ使う |

- 台詞の数の目安: 台詞を出す situation ごとに 5 本以上(連打で直近 `TAP_HISTORY` = 4 件を避けても候補が残るようにする。AC12)。合計で 45 本前後。
- 禁止語は `lines.rs:649-656` と同じ一覧をテストに複製する(`SAD` `PET ME` `PLEASE` `FEED ME` など)。`sad` 状態の台詞でも `SAD` という語は使わない(`A BIT GLUM...` などにする)。

### 4.3 連打の扱い(`acceptTap`)

メモリ上に小さな状態 `TapState = { lastAt, streak: [時刻...], restUntil, recentIds: [] }` を持ち、純関数で次の状態を返す。

```js
/**
 * Whether a tap at `now` (ms, monotonic) counts, and the state after it.
 * kind: 'ignored' | 'normal' | 'dizzy' | 'resting'
 */
export function acceptTap(state, now) -> { kind, state }
```

規則(上から順に評価する):
1. `state.lastAt` があり、`0 <= now - lastAt < TAP_MIN_GAP_MS` なら `ignored`(状態は変えない)。`now < lastAt`(時計が戻った)場合は、十分前とみなす。
2. `now < restUntil` なら `resting`(瞬きだけで台詞なし)。`lastAt` は更新し、`streak` には足さない。
3. `streak` から `TAP_STREAK_WINDOW_MS` より古いものを捨て、`now` を足す。件数が `TAP_STREAK_DIZZY` 以上なら `dizzy`。`restUntil = now + TAP_REST_MS`、`streak = []` にする。
4. それ以外は `normal`。

| 定数 | 初期値 | 安全範囲 | 上げると | 下げると |
|---|---|---|---|---|
| `TAP_MIN_GAP_MS` | 700 | 400–1500 | ゆっくり押しても無視される | 反応が重なってせわしない(CLAUDE.md の「calm」) |
| `TAP_STREAK_WINDOW_MS` | 5000 | 3000–10000 | ゆっくりした連続も「連打」になる | 目が回る反応が出にくい |
| `TAP_STREAK_DIZZY` | 5 | 4–8 | 目が回る反応が稀になる | 普通に数回触っただけで目が回る |
| `TAP_REST_MS` | 10000 | 5000–30000 | 休みが長く、触っても反応が薄い時間が増える | 連打を止める効果が弱い |
| `TAP_REACT_MS` | 600 | 300–1000 | 動きが長く目立つ | 動きが見えない |
| `TAP_SPEAK_P` | 0.6 | 0.3–1.0 | おしゃべりになる | 触っても黙っていることが多い |
| `TAP_HISTORY` | 4 | 2–6 | 候補が尽きやすい | 同じ台詞が続く |

- `TAP_MIN_GAP_MS = 700` は `TAP_REACT_MS = 600` より長くして、反応が重ならないようにした。
- **最悪の場合の台詞の量**: 700ms ごとに押し続けると、5 回目(2.8 秒後)で目が回り、続く 10 秒は台詞が出ない。1 周期 12.8 秒のうち台詞は、`normal` 4 回 × 0.6 + `dizzy` 1 回 = 約 3.4 本。1 分あたり約 16 本、平均で約 3.8 秒に 1 本になる。普通に触る程度(数秒に 1 回)なら、1 回あたり 0.6 本。これより静かにしたい場合は `TAP_SPEAK_P` か `TAP_STREAK_DIZZY` を下げる。
- 状態は、ポップオーバーを閉じても `TapUI` のインスタンスに残る(ページは再読み込みされない)。ただし `blur` で `streak` と `restUntil` を消し、次に開いたときに休みが残らないようにする。`recentIds` は残す。

### 4.4 状態別の反応表(`tapSituation`)

```js
/** The situation the tap is read in, from the pet, the face mood and the local hour. */
export function tapSituation({ pet, mood }) -> situation | null
```

上から最初に一致したものを使う。`mood` は `CompanionUI.mood()` の値(生きている大人で companion が有効なときだけ文字列、それ以外は null)。時間帯は situation ではなく、台詞の `part` で効かせる(§4.5)。

| 優先 | situation | 条件 | 動き(`motion`) | 表情(`face`) | 台詞 | 台詞の例 |
|---|---|---|---|---|---|---|
| 1 | (null) | `!pet` または `!pet.is_alive` | なし | なし | なし | — |
| 2 | `egg` | `stage == 'Egg'`(不明な段階も Egg として描くので、ここに含める) | `wobble`(揺れを 1 回だけ大きくする) | なし(目がない) | なし | — |
| 3 | `sleeping` | `activity == 'Sleeping'` | なし(起こさない) | 大人: `closed`、他: 目を線にする | 必ず(`TAP_SPEAK_P` を使わない) | `ZZZ...` `ZZ...` `MM...` `SNORE...` `ZZZ. ZZZ.` |
| 4 | `busy` | `activity` が `Eating` か `Playing` | `hop`(小さく) | 変えない | 確率 P | `NOM.` `BUSY.` `ONE SEC.` `HM?` `MID-BITE.` |
| 5 | `hungry` | 大人で `mood == 'concerned'` かつ最小の need が hunger、または `status == 'hungry'` | `droop`(1px 沈む) | 大人: `concerned` | 必ず | `TUMMY RUMBLES...` `HUNGRY...` `COULD EAT...` |
| 6 | `tired` | 同じく energy、または `status == 'tired'` | `droop` | 大人: `sleepy` | 必ず | `SO TIRED...` `EYES HEAVY...` |
| 7 | `glum` | 同じく happiness か health、または `status == 'sad'` | `droop` | 大人: `concerned` | 必ず | `A BIT GLUM...` `NOT MY DAY...` |
| 8 | `baby` | `stage == 'Baby'` | `hop` | 目を線にする(にっこり) | 確率 P | `BA?` `DA.` `MU.` `HEHE.` `OO?` |
| 9 | `child` | `stage` が `'Child'` か `'Teen'` | `hop` | 目を線にする | 確率 P | `HI.` `HEHE.` `THAT TICKLES.` `WHAT?` `HM HM.` |
| 10 | `adult` | `stage == 'Adult'` | `hop`(`mood == 'sleepy'` なら `droop`) | `mood` が `cheerful` / `calm` / `missed_you` なら `cheerful`、`sleepy` なら `sleepy`。companion OFF(mood が null)なら `blink` | 確率 P | cheerful: `HEHE. HI.` / calm: `HM?` `I'M HERE.` / sleepy: `YAWN...` / missed_you: `OH. HELLO.` / 夜: `STILL UP?` |

- 連打の結果で上書きする:
  - `ignored`: 何もしない。
  - `resting`: 表情 `blink` だけを返す(動き・台詞なし)。situation が null なら何もしない。Egg では何もしない(目がない)。
  - `dizzy`: situation が `baby` / `child` / `adult` のときだけ、動き `wobble`・表情 `blink` に加えて `situation: 'dizzy'` の台詞(`DIZZY...` `WOOZY...` `ROUND AND ROUND...` `OK, OK.` `SPINNY...`)を必ず出す。`egg` は `wobble` だけ。`sleeping` / `busy` / `hungry` / `tired` / `glum` は目を回さず、`normal` と同じに扱う(具合の悪い dot を目回しにしない)。
- 大人の `hungry` / `tired` / `glum` の判定: companion が有効なら、Mood が `concerned` かを見たうえで、pet の値から最小の need を選ぶ(`mood.rs:156-168` と同じ順: hunger → happiness → energy → health。値の比較だけで閾値は使わない)。companion が OFF の大人と Baby / Child は、バックエンドが決めた `pet.status`(`state.rs:262-271`)を使う。**閾値を JS に複製しない**ための方針。
- 台詞を「必ず」出すのは、dot 自身の状態の表明は黙らない方が伝わるため。ただし連打の規則(`ignored` / `resting`)は「必ず」より優先する。

### 4.5 台詞の選択(`pickTapLine`)

```js
/**
 * The line for a tap, or null for silence. `random` returns [0, 1) and is
 * passed in so tests are repeatable.
 */
export function pickTapLine({ lines, situation, mood, part, recentIds, mustSpeak, random }) -> line | null
```

1. `mustSpeak` でなければ、`random() < TAP_SPEAK_P` のときだけ先へ進む。外れたら null。
2. 候補は、`lines` のうち `situation` が一致し、`mood` の指定があればそれも一致し、`part` の指定があればそれも一致し、`recentIds` に含まれないもの。
3. 候補が空なら、`recentIds` を無視して 2 をやり直す(連打で尽きても黙り込まないように)。それでも空なら null。
4. `candidates[Math.floor(random() * candidates.length)]` を返す。

- `part` は `partOfHour(hour)` で求める。境界は `Part::of_hour`(`lines.rs:64-71`)と同じ(05–10 / 11–16 / 17–21 / 22–04)。`part` 付きの台詞は、その時間帯なら `part` なしの台詞と同じ重みで候補に加わる。
- 呼び出し側(`TapUI`)は、返った台詞の ID を `recentIds` の末尾に足し、`TAP_HISTORY` 件を超えた分を先頭から捨てる。これも純関数 `rememberTap(recentIds, id)` にする。
- `random` は本番では `Math.random` を渡す。

### 4.6 動きと表情(`tapMotion` と renderer)

```js
/** The offset of a reaction `t` ms after it began, or null once it is over. */
export function tapMotion(motion, t, { reducedMotion }) -> { dx, dy } | null
```

- `hop`: `dy = -2 * sin(π · t / TAP_REACT_MS)`(最大 2px 上)。既存の Baby の跳ね(最大 3px、`renderer.js:173`)より小さくする。
- `droop`: `dy = +1`(反応の間だけ 1px 沈む)。
- `wobble`: `dx = 2 * sin(4π · t / TAP_REACT_MS)`(いつもの揺れに 1 回だけ上乗せする)。
- `reducedMotion` のときは常に `{ dx: 0, dy: 0 }`(AC17)。`t >= TAP_REACT_MS` で null を返す。
- `PetRenderer.react({ motion, face, until })` は値を保存するだけ。各 `drawXxx` は `translate` にオフセットを足し、`until` までは表情を差し替える。
  - 大人: `this.face` より反応の表情を優先する(`renderer.js:290-292` の `shape` を決める処理に 1 段足す)。companion OFF の大人も、反応の間だけ `drawAdultEye` を使う。
  - Baby / Child: 目を描く部分(`renderer.js:198-205`, `242-249`)に「線の目」(白い 4×4 を描かず、黒い 4×2 の横線にする)の分岐を足す。
  - 反応の終わりは、描画のたびに `performance.now() >= until` で判定する。タイマーは使わない(AC18)。
- `setFace` の瞬き(`renderer.js:331-333`)と反応が重なった場合は、反応を優先する。

### 4.7 他の吹き出し・コンパニオンとの関係

- `CompanionUI` に `say(rows)` を足す。中身は `showBubble({ kind: 'line', rows })` に `source: 'tap'` の印を付けたもの。`isShowingOwnBubble()` は、いま出ている吹き出しが `companion_open` 由来(台詞または prompt)なら true を返す。
- `TapUI` は、`isShowingOwnBubble()` が true なら台詞を出さず、動きと表情だけを返す(AC13)。理由は、開いたときの 1 行(R3「開くたびに 0 または 1 行」)と、オンボーディングの回答ボタンを失わないため。タップの吹き出しは、次のタップの吹き出しで置き換えてよい。
- 開いた直後のクリック: `companion_open` は非同期で、結果が来ると `dismiss()` してから台詞を出す(`companion.js:105-109`)。その前に出ていたタップの台詞は消えるが、「開いたときの 1 行が優先」という同じ方針に合うので許容する。
- muted: `CompanionUI.open()` の後に `getCompanionSettings()` を 1 回呼び、純関数 `isMuted(settings, nowSecs)`(`!master_on || quiet_until > now`)の結果を保持する。設定を変えたとき(`saveSettings`)にも更新する。muted ならどの段階でも台詞を出さない(AC14)。取得に失敗したら muted でないとみなす(台詞が出るだけで害はない)。
- Farewell: situation が null なので何もしない。`#companion-bubble` も `display: none`(`main.css:292`)なので、二重に安全。
- タップの吹き出しは `companion_open` 由来ではないので、Bond・`LineHistory`・`today.g` には触れない(AC4)。

### 4.8 アクセシビリティ

- キャンバスを `role="button" tabindex="0" aria-label="dot"` にする。`aria-label` は固定にする(段階や状態は情報行 `#pet-info` で読める)。
- Enter / Space の keydown でタップと同じ処理をする。Space はページのスクロールを防ぐため `preventDefault()` する。キーの自動リピートは `TAP_MIN_GAP_MS` で自然に間引かれる。
- `:focus-visible` で 2px の点線の枠を出す(白黒の方針に合わせる)。マウスでクリックしたときは枠を出さない。
- 台詞は既存の live region(`#companion-bubble`, `role="status"`, `aria-live="polite"`)に入るので、読み上げられる。`resting` や台詞のない反応では DOM を変えないので、読み上げない。
- `prefers-reduced-motion`: 位置の動きと瞬きを止め、表情の差し替えと台詞は残す(GDD Game Feel の「瞬きと揺れを止めて静止画にする」と同じ扱い)。吹き出しは既存どおり、フェードせず即座に消える(`companion.js:201-204`)。

### 4.9 GDD(`design/gdd/companion.md`)との整合

| GDD の項目 | 本設計での扱い |
|---|---|
| R1 Companion は生きている大人だけ | 変えない。タップ反応は Companion の機能ではなく、全段階向けの別機能とする。大人のときだけ Mood を読む |
| R2 Mood | 読むだけで変えない(AC9)。タップで Mood を計算し直さない |
| R3 開くたびに 0 または 1 行 | 開いたときの行は変えない。タップの行は別枠で、開いたときの行が出ている間は出さない(§4.7) |
| R3 / Player Fantasy のトーン規約(大文字・24 文字・命令形なし・感嘆符なし) | 同じ規約を node のテストで検査する |
| R6 Bond / P3「連打で絆が上がる → 却下」 | タップは Bond に数えない |
| R11 Companion はコインを生まない | タップもコイン・ステータスを変えない |
| `muted` フラグ | 台詞を止める(全段階)。動きは止めない |
| `quiet_hours` | 台詞を止めない(ユーザーが自分で触ったため) |
| アンチピラー 3(罪悪感を使わない) | 禁止語の検査を共有する。空腹などは dot 自身の状態の表明に留める |

GDD への追記(「R12 タップ反応」として、本書へのリンクと上の表の要点)は、実装 PR で行う(§7 の T7)。

---

## 5. テスト計画

`node --test tests/*.test.mjs` で実行する。新しく作る `tests/tap-logic.test.mjs` に以下を書く。DOM の部分(`tap.js`・renderer)は手動で確認する。

### 5.1 `acceptTap`(連打)
- 初回は `normal`。
- 699ms 後は `ignored` で状態が変わらない。700ms 後は `normal`。
- 700ms 間隔で 5 回押すと 5 回目が `dizzy`。その後 10 秒未満は `resting`、10 秒後は `normal`。
- 5 秒の窓から外れた古いタップは数えない(1.5 秒間隔で 5 回押しても dizzy にならない)。
- 時計が戻った(`now < lastAt`)ときは `ignored` にしない。
- `resting` 中のタップは `streak` に足さない(休み明けすぐに dizzy にならない)。

### 5.2 `tapSituation`(状態表)
- §4.4 の各行を 1 本ずつ: 死亡 → null、Egg、不明な段階 → egg、Sleeping、Eating / Playing、hungry / tired / sad(status 経由)、Baby、Child、Teen → child、Adult。
- 優先順の重なり: 睡眠中かつ空腹 → `sleeping`、食事中かつ空腹 → `busy`、死亡かつ睡眠中 → null。
- 大人で mood が `concerned`: hunger が最小 → `hungry`、energy が最小 → `tired`、happiness / health が最小 → `glum`。同じ値なら hunger → happiness → energy → health の順(`mood.rs:156-168` と同じ)。
- 大人で mood が null(companion OFF): `status` で判定する。
- 連打の上書き: `dizzy` は baby / child / adult だけ、`sleeping` / `hungry` などでは `normal` 扱い、`resting` は blink だけ。

### 5.3 `pickTapLine` / `rememberTap`
- `random` に固定の値の列を注入して、決定的にする。`mustSpeak: false` で `random() >= TAP_SPEAK_P` なら null。
- `mood` / `part` 付きの台詞は、条件が合うときだけ候補になる。
- `recentIds` にある台詞は出ない。候補が全部履歴にある場合は、履歴を無視して選ぶ。
- 100 回選んで、直近 4 件との重複がない(候補が 5 件以上の situation で)。
- `rememberTap` は 4 件を超えた分を先頭から捨てる。

### 5.4 `tapMotion`
- `reducedMotion: true` なら常にオフセット 0。
- `t >= TAP_REACT_MS` で null。`hop` は最大 2px を超えない。

### 5.5 台詞データ(`TAP_LINES`)の検査(`lines.rs:666-790` に揃える)
- ID が一意で、`tap_` で始まり、小文字・数字・`_` だけ。
- `text` は 24 文字以内で、`splitLine` で 2 行以内に収まる。文字種は大文字・数字・空白・`.,?'-` だけ(感嘆符なし)。
- 禁止語(`lines.rs:649-656` と同じ一覧)を単語単位で含まない。
- `situation` / `mood` / `part` の値が既知の名前だけ。
- 台詞を出す situation(`sleeping` `busy` `hungry` `tired` `glum` `baby` `child` `adult` `dizzy`)ごとに 5 本以上。`adult` は Mood ごとに 1 本以上。

### 5.6 その他
- `partOfHour` の境界: 4→night、5→morning、10→morning、11→day、16→day、17→evening、21→evening、22→night(`lines.rs:633-643` の期待値と同じ)。
- `isMuted`: マスター OFF、`quiet_until` が未来 / 過去 / null。
- 既存の `tests/companion-logic.test.mjs` が変更なしで通ること(`say` を足しても `bubbleContent` などの挙動は変えない)。
- Rust は変更しないので、`cargo test` は回帰確認として実行するだけ。

### 5.7 手動確認(`design/issue-53-manual-check.md` を実装 PR で作る)
- 各段階で(セーブを書き換えるか DevTools で状態を用意して)、クリック・Enter・Space を試す。
- 睡眠中にクリックしても、`activity` と残り時間が変わらない。
- 連打すると目が回り、10 秒休む。
- 開いた直後の台詞と `STAY WITH YOU?` が置き換わらない。
- VoiceOver で台詞が読まれる。システム設定の「視差効果を減らす」で動きが止まる。
- FAREWELL・GAME OVER でクリックしても何も起きない。
- クリックしても `pet.json` / `companion.json` の更新時刻が変わらない。

---

## 6. リスク

| # | リスク | 影響 | 対策 |
|---|---|---|---|
| 1 | 台詞データが `lines.json` と `tap-lines.js` に分かれ、トーンや禁止語の規則がずれる | 罪悪感を誘う台詞や命令形の台詞が紛れ込む | node のテストで同じ規則を検査する(§5.5)。Rust 側の禁止語一覧のコメントに「tap-lines の検査にも複製あり」と書く |
| 2 | `CompanionUI.syncFace()` が 5 秒ごとに `setFace` を呼び、反応の表情を上書きする | 反応が途中で消える | renderer 側で、反応の表情を `this.face` より優先する(§4.6)。上書きではなく重ね描きにする |
| 3 | 開いた直後のクリックで、`companion_open` の結果がタップの台詞を消す | タップの台詞が一瞬で消える | 「開いたときの 1 行が優先」として仕様にする(§4.7)。気になる場合は判断事項 4 で見直す |
| 4 | キャンバスに tabindex を付けると、ポップオーバーを開いたときのフォーカス順が変わる | 既存のキーボード操作の順序が変わる | `autofocus` は付けない。Tab の順は DOM 順(`*` メニューの後、FEED の前)で自然になる |
| 5 | 連打でせわしなく見える(CLAUDE.md の「calm, not distracting」) | 世界観が崩れる | 動きは最大 2px・600ms。700ms 未満は無視し、5 回で休む |
| 6 | 大人の空腹などの判定に、companion ON では Mood、OFF では status と 2 系統を使う | 境界(hunger 20–29 でヒステリシス中)で、ON と OFF で台詞が変わる | 意図した差(ON のときは画面の Mood と一致させる)として、テストで固定する |
| 7 | muted を知るために、open のたびに `get_companion_settings` が 1 回増える | IPC 1 回分 | open のときだけで、軽いコマンドなので許容する。気になる場合は `CompanionOpenResult` に `muted` を足す(Rust の変更が要る。判断事項 6) |
| 8 | 睡眠中の絵がない(§1.2)ため、Baby / Child の睡眠中は `ZZZ...` だけが出て、絵とずれて感じる | 小さな違和感 | 反応の間だけ目を線にする(§4.4)。睡眠の絵そのものは別 Issue にする |
| 9 | 吹き出しが目の上に出るので、キャンバスの上側が吹き出しで覆われる | 吹き出しの上をクリックすると、タップではなく吹き出しの消去になる | 既存の吹き出しと同じ挙動。吹き出しはクリックで消えるので、次のクリックはキャンバスに届く |
| 10 | `Teen` などの旧段階や、将来の段階 | 反応がない、または誤る | 描画と同じ規則(`renderer.js:122-130`: Teen は Child、不明な段階は Egg)に揃える |

---

## 7. タスク分割

各タスクは 1 コミット程度。T1–T3 はテストを先に書く。

| ID | 内容 | 変更ファイル | 依存 |
|---|---|---|---|
| T1 | 連打と状態表の純関数: `acceptTap`、`tapSituation`、`partOfHour`、`isMuted`、定数。テストは §5.1 / §5.2 / §5.6 | `frontend/js/tap-logic.js`、`tests/tap-logic.test.mjs` | — |
| T2 | 台詞データと選択: `tap-lines.js`、`pickTapLine`、`rememberTap`。テストは §5.3 / §5.5 | `frontend/js/tap-lines.js`、`tap-logic.js`、テスト | T1 |
| T3 | 動き: `tapMotion` とテスト §5.4。`PetRenderer.react()` と、各段階の描画への反映(オフセット・線の目・大人の表情の優先) | `tap-logic.js`、`renderer.js`、テスト | T1 |
| T4 | 吹き出しの共有: `CompanionUI.say()`、`isShowingOwnBubble()`、open 後の設定取得と muted の保持 | `companion.js` | — |
| T5 | 組み立て: `TapUI`(click / keydown、blur で連打の状態を消す)と、`main.js` での生成 | `frontend/js/tap.js`、`main.js` | T1–T4 |
| T6 | アクセシビリティとスタイル: `role` / `tabindex` / `aria-label`、`cursor: pointer`、`:focus-visible` | `index.html`、`main.css` | T5 |
| T7 | ドキュメント: GDD に「R12 タップ反応」(本書へのリンクと §4.9 の要点)、手動確認リスト、CHANGELOG | `design/gdd/companion.md`、`design/issue-53-manual-check.md`、`CHANGELOG.md` | T5 |

---

## 8. レビューしてほしい判断事項

1. **方式**: バックエンドの台詞基盤を流用する(案 A)か、フロントだけの軽量な仕組み(案 B)か。
   - **推奨: 案 B。** Companion の R1(大人だけ)とセーブを変えずに全段階へ広げられ、IPC も永続化もない。台詞データが 2 か所になる点は、node の検査で補う。
2. **対象の成長段階**: 大人だけか、全段階か。
   - **推奨: Egg を含む全段階。** ただし Egg は台詞なしで揺れだけ、Baby は 1 語の喃語、Child は短い言葉、Adult は Mood に合った言葉、と段階ごとに言葉を増やす(育っている実感にもなる)。
3. **死亡中の反応**: GAME OVER / FAREWELL でクリックしたら反応するか。
   - **推奨: 何もしない。** FAREWELL は目を閉じた静止画で別れを表す画面で、触って動くと演出が崩れる。GAME OVER ではキャンバスが覆われている。
4. **開いたときの台詞や `STAY WITH YOU?` が出ているときのタップ**: 置き換えるか。
   - **推奨: 置き換えず、動きと表情だけを返す。** 開いたときの 1 行(R3)と回答ボタンを失わないため。吹き出しはクリックで消せるので、消した後のタップには台詞が出る。
5. **睡眠中の反応**: 起こすか、寝言を言うか、何もしないか。
   - **推奨: 起こさず、`ZZZ...` 系の台詞だけを出す(動きなし)。** ステータスを変えない前提に合い、睡眠中でも触った手応えがある。
6. **muted(マスター OFF / QUIET)の扱いと取得方法**: タップの台詞を止めるか。止めるなら、設定をどう知るか。
   - **推奨: 全段階で台詞を止め、動きは残す。** 「dot に黙っていてほしい」というユーザーの明示の意思を尊重する。設定は open の後に既存の `get_companion_settings` を 1 回呼んで知る(Rust の変更なし)。静寂時間(22–08)では止めない(GDD の `quiet_hours` と同じ)。
7. **連打の扱い**: 無視するか、反応を変えるか。
   - **推奨: 両方。** 700ms 未満は無視し、5 秒に 5 回で一度だけ `DIZZY...` と言い、その後 10 秒は瞬きだけを返す。具合が悪いとき(空腹など)・睡眠中・食事中は目を回さない。
8. **台詞を出す確率**: 毎回話すか。
   - **推奨: 通常は 60%(`TAP_SPEAK_P`)。空腹など・睡眠中・目が回ったときは必ず話す。** GDD の「沈黙も表現」(Game Feel)に合わせつつ、触った手応えは動きで毎回返す。
9. **大人期の空腹などの判定**: Mood(companion)と `pet.status` のどちらを使うか。
   - **推奨: companion が有効な大人は Mood(画面の表情と一致させる)、それ以外は `pet.status`。** どちらもバックエンドが決めた値なので、閾値を JS に複製せずに済む。
10. **吹き出しの表示時間**: タップ用に短くするか。
    - **推奨: 既存の `BUBBLE_TTL_MS`(6 秒)をそのまま使う。** 読み上げや読む速さへの配慮を既存と揃え、ノブを増やさない。次のタップで置き換わるので、長すぎて困ることは少ない。
11. **キャンバスをキーボード操作の対象にするか**。
    - **推奨: する(`role="button"`、`tabindex="0"`、Enter / Space)。** マウスだけの機能にしない。`autofocus` は付けず、既存のフォーカス順を乱さない。
12. **GDD への反映**: `design/gdd/companion.md` に追記するか、本書だけで持つか。
    - **推奨: 実装 PR で GDD に短い「R12 タップ反応」を足し、詳細は本書へリンクする。** Companion の R3(開くたびに 1 行)と muted の解釈に関わるので、GDD 側からも辿れるようにする。

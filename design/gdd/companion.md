# Companion(大人期コンパニオン)

> **Status**: In Design — Review 1(NEEDS REVISION)の指摘 16 件を反映済み、再レビュー待ち(`design/gdd/reviews/companion-review-log.md`)
> **Author**: Keitinn + Claude(creative-director / economy-designer / ux-designer / technical-director / systems-designer 協議)
> **Last Updated**: 2026-10-14
> **Last Verified**: 2026-10-14(コード基準: commit `3167e43`)
> **Implements Pillar**: P1 静かな同居 / P4 世話は双方向(いずれも推定)
> **Creative Director Review (CD-GDD-ALIGN)**: NOT ASSESSED 2026-10-14 — solo モードのため正式ゲート未実施(Player Fantasy 策定時に creative-director へ個別相談済み)

## Summary

大人(Adult)になった dot が、「世話をされるペット」から「あなたが戻るのを待っていて、さりげなく気遣う相棒」へ役割を変えるシステム。時刻・離席・ペットの状態から気分(Mood)を決め、ポップオーバー内の短い一言と静止画のトレイ表情でだけ表現する。OS 通知・音・点滅は既定で使わない。長期の動機は数値を見せない絆(Bond)と、1 行日記「MEMORIES」で支える。

> **Quick reference** — Layer: `Feature` · Priority: `Post-MVP`(推定: systems index なし) · Category: `Gameplay`(推定: systems index なし、README/CLAUDE.md から導出) · Key deps: `Pet State/Needs`, `Evolution`, `Lifecycle(死と復活)`, `Storage`, `GameStats/Economy`, `Achievements`, `Renderer/Tray`

---

## 0. 前提(推定)と採用した仮定

### 0.1 本 GDD の運用上の注記
- CCGS 設定は `workflow: minimal / review_mode: solo / automation: collaborative`。minimal では GDD は任意で、標準 5 セクションが既定。**依頼者の指定に従い 8 セクション(full 相当)で作成**した。
- `design/gdd/game-concept.md`・`systems-index.md`・`game-pillars.md`・`design/registry/entities.yaml` は存在しない。ゲーム全体の GDD は作らず、必要最小限の前提を以下に「推定」として置く。
- エンジン参照(`docs/engine-reference/`)はない。Tauri v2 の実現性は technical-director の調査のみで、エンジン参照による検証はしていない。
- 無人実行のため AskUserQuestion には回答が得られない。**各判断点では推奨案を採用した**(§0.4)。最終判断は末尾「レビュアーへの決定事項」で依頼する。
- ソースコードは変更していない。CCGS のセッション状態ファイル(`production/session-state/active.md`)は design/ 外なので作成していない。

### 0.2 推定コンセプト(README.md / CLAUDE.md より)
- macOS メニューバーに住む、白黒ピクセルのたまごっち風ペット(README.md:5, CLAUDE.md:3)。
- 現行のコアループ: FEED/PLAY/SLEEP で世話をする → ステータスが減衰 → 年齢で進化(Egg→Baby→Child→Adult) → 放置すると死亡 → NEW PET。コイン・実績・ストリークはペットをまたいで残る。
- 大人期の構想(CLAUDE.md:31-41): 「世話をするペット」から「あなたを気遣う相棒」へ。

### 0.3 推定ピラー(creative-director 案を採用)
| # | ピラー(推定) | 一行定義 | 判定例 |
|---|---|---|---|
| P1 | 静かな同居 | 邪魔をしない。存在を強く示すのはユーザーが開いたときだけ | 「トレイを点滅させて気づかせる」→ 却下 |
| P2 | 少ないもので伝える | 白黒ドットと目の表情だけで感情を伝える。足すより引く | 色付きアクセサリ → 却下 |
| P3 | 時間が関係を育てる | 価値は一緒に過ごした時間から生まれる。操作量では育たない | 連打で絆が上がる → 却下 |
| P4 | 世話は双方向 | 最初はユーザーが世話をし、やがて dot がユーザーを見守る | 生産性スコアの表示 → 却下(見守るのであって評価はしない) |

**アンチピラー(dot が絶対にしないこと)**
1. OS 通知・音・トレイの点滅で呼び出さない(通知は Ambitious 案のオプトインのみ、既定 OFF)。
2. 生産性を評価しない(「今日は○時間作業した」は言わない)。
3. 罪悪感を利用しない(「寂しかった…」で不在を責めない)。
4. キー入力の内容・回数、画面、カレンダー本文を読まない。
5. 連続ログインを罰しない。

### 0.4 採用した仮定(AskUserQuestion の代わり)
| # | 判断点 | 採用案(推奨) | 却下した主な代替案 |
|---|---|---|---|
| A1 | Overview の書き方 | 体験と仕組みの両方を書く | 技術面のみ |
| A2 | Player Fantasy | B「帰ると待っている存在」を核に、C「小さな観測者」の言葉づかいを混ぜる | A「隣の席の同僚」(生産性アプリの色が出る) |
| A3 | 絆の見せ方 | 内部値として持ち、数値やゲージは見せない | 数値を表示 |
| A4 | 大人期の減衰 | 現行の 0.4 倍 | 現状のまま / 減衰なし |
| A5 | 大人期の死 | 死はありうるが、約 42 時間(1.7 日)以上の完全放置が必要(現行 Child は約 17 時間)。演出は「別れ」 | 床値を設けて死なない |
| A6 | 復活時の絆 | 新しいペットは絆 0 から。前のペットの MEMORIES はアルバムに永久保存 | 25% を引き継ぐ |
| A7 | コイン経済との接続 | **新しい稼ぎ口(ファウセット)は作らない**。MVP ではシグナル追加もしない。Standard で小さな見た目アイテムを使い道(シンク)として追加するかはレビュアーが判断 | 活動に応じてコインを付与 |
| A8 | 通知 | OS 通知は MVP・Standard では作らない | 進化時に通知の許可を求める |
| A9 | 打鍵検知 | 採用しない(Never) | Input Monitoring 権限で取得 |
| A10 | カレンダー | Ambitious 案のオプトインまで延期 | MVP に含める |
| A11 | 静寂時間の既定値 | 22:00–08:00 | 22:00–07:00 |

---

## Overview

Companion は、ペットが **Adult に到達した時点で有効になる**レイヤー。既存の世話ループ(`backend/src/pet/state.rs:100-185`)はそのまま残し、意味づけを変える。

- **世話を義務から挨拶へ**: 大人期は needs の減衰を 0.4 倍にし、世話は 1 日 1〜2 回で足りるようにする。
- **dot がユーザーに反応する**: 時刻・ユーザーの不在時間・ペット自身の状態から Mood を決める。ユーザーがポップオーバーを開いたときに 1 行の言葉(例: `YOU'RE BACK.` `LATE.` `I'M HERE.`)と表情で迎える。
- **見えない絆**: 一緒に過ごした日々が内部値 Bond として貯まる。絆が深まるほど表情や台詞の種類が増え、MEMORIES(日付付きの 1 行日記)が解放される。

今の dot は Adult が終点で、到達後は何も起きない(`backend/src/pet/evolution.rs:55`)。このシステムがないと、育成の到達点に報酬がなく、大人になった後に開く理由も消える。役割は「ケアの対象」から「そばにいる相棒」への転換であり、生産性ツールではない(アンチピラー 2)。

## Player Fantasy

**「帰ると、待っていてくれる」**(A2)。

- **核となる瞬間**: 深夜、仕事の合間にメニューバーの dot を開く。眠そうな目の dot がこちらを見て、`…LATE.` とだけ言う。叱りもせず、褒めもしない。気づいていて、そこにいるだけ。
- **感情**: 安心感と、覚えていてもらえる嬉しさ。数週間後に `DAY 21 — RAINY MONDAY` のような思い出を見つけて、関係が積み重なっていたことに気づく。
- **トーン規約**:
  - 英語大文字で 1〜3 語、最大 24 文字(既存 UI は大文字の英語ピクセル表記: `frontend/index.html:72-74`)。
  - 命令形を使わず、観察や気持ちの表明だけにする(`REST?` ではなく `I'M HERE.`)。
  - 感嘆符は 1 日 1 回まで。絵文字は使わない。
  - 不在のあとは `WELCOME BACK.` だけを言い、不在を責めない。
- **参照作品**: たまごっち(小さな画面の存在感)、どうぶつの森(友好度を見せずに関係を伝える)。

---

## Detailed Design

### Core Rules

**R1. 有効化**
1. `pet.stage == Adult && pet.is_alive` のときだけ Companion は有効になる(`evolution.rs:4-10`)。
2. Adult 到達後に初めてポップオーバーを開いたとき、dot の吹き出しでオンボーディングを出す: `STAY WITH YOU?` `[YES] [LATER]`。モーダルは使わない。
   - `YES`: Standard 機能の離席検知とトレイ表情を有効にする。
   - `LATER` または無視: MVP の範囲(ポップオーバー内の反応のみ)で動く。`LATER` は 7 日後に 1 回だけ再提示し、その後は設定からのみ変更できる。
3. 設定(`*` メニュー: `frontend/index.html:17-21`)に `COMPANION` 項目を追加する。既定値は Tuning Knobs の表どおり。
   - `COMPANION` のマスター ON/OFF(既定 ON)
   - `TRAY FACE` の ON/OFF(既定は YES 回答時に ON)
   - 静寂時間(既定 22–08)
   - `QUIET 1H` / `QUIET TODAY`(手動の静音)

**R2. Mood の決定**(優先順位表で上から最初に一致したものを採用。式は Formulas F2)

| 優先 | Mood | 条件 | 表情(白黒) |
|---|---|---|---|
| 1 | `concerned` | needs の最小値 < 20、または health < 30 | 目を下げる |
| 2 | `sleepy` | 睡眠中、energy < 35、または 23–06 時 | 半目 |
| 3 | `missed_you` | 最後に開いてから 24 時間以上(開いた瞬間だけ評価し、表示後に解除) | 目を大きく、瞬き 1 回 |
| 4 | `calm` | フォーカス中、または needs の最小値 < 60 | 通常の目 |
| 5 | `cheerful` | 上記以外 | 目を弧にする |

- systems-designer 案の `lonely` は、罪悪感を誘う(アンチピラー 3)ため `missed_you` に改名した。演出は嬉しさ寄りにする。
- `concerned` は世話を促すが、言葉は `HUNGRY…`(dot 自身の状態の表明)に留め、ユーザーへの命令にはしない。

**R3. 言葉(Line)の選び方**
1. ポップオーバーを開くたびに、0 または 1 行を表示する。開くこと自体はユーザーの意思なので割り込みではないが、繰り返しを避けるためにクールダウンを設ける。
2. 台詞はカテゴリ別のプールから選ぶ。候補カテゴリを次の優先順に評価し、最初に「条件成立 かつ クールダウン明け」のものを 1 つだけ採用する。

   | 優先 | カテゴリ | 条件 | 発話 |
   |---|---|---|---|
   | 1 | `greet_first_today` | その日最初の手動オープン | 必ず |
   | 2 | `mood_concerned` | Mood が `concerned` | 必ず(同じ need については 6 時間に 1 回) |
   | 3 | `missed_you` | 前回オープンから 24 時間以上 | 必ず |
   | 4 | `return_after_away` | 前回オープンから 4 時間以上、または(Standard)10 分以上の離席から復帰 | 必ず |
   | 5 | `battery`(Standard) | 非充電中で 20% 未満。50% を超えるまで再発しない | 必ず |
   | 6 | `bond_tier` | ティア到達後、最初のオープン(1 回のみ) | 必ず |
   | 7 | `time_of_day` | 朝/昼/夕/深夜の時間帯 | 確率 LINE_SPEAK_P |
   | 8 | `mood` | 現在の Mood(`concerned` 以外) | 確率 LINE_SPEAK_P |

   - 候補がすべて不成立、またはクールダウン中なら 0 行(沈黙)になる。
   - 乱数はシード可能にし、テストで決定的に再現できるようにする。
3. 直近 LINE_HISTORY=20 件と同じ ID は出さない。同じカテゴリは LINE_CATEGORY_COOLDOWN=24 時間に 1 回まで。例外は優先 1 と 2(それぞれの条件側で回数を制限している)。
4. 吹き出しはクリックで消える。6 秒で自動フェードし、ホバー中は消えない。ポップオーバーを閉じたら破棄し、未読として溜めない。
5. MVP の台詞は最低 40 本(各カテゴリ 6 本以上)。台詞はデータ駆動にし、コードに直書きしない。

**R4. 不在と復帰**(MVP は不在検知の API を使わない)
- MVP: 前回ポップオーバーを開いた時刻からの経過で判定する。4 時間以上なら `return_after_away`、24 時間以上なら `missed_you`。
- Standard: Rust 側で権限不要のアイドル時間(`CGEventSourceSecondsSinceLastEventType`)を 60 秒ごとに読む。10 分以上の離席から戻ったら、トレイを「話したい」表情に変える(F3 の予算を消費)。キー入力の内容や回数は取らない。

**R5. 大人期の減衰**
- Adult の間、hunger・happiness・energy の減衰率に `ADULT_DECAY_MULT = 0.4` を掛ける(`needs.rs:4-6`)。health の増減ルールは変えない。
- 前提: 端数の切り捨てバグ(前提タスク P2)が修正されていること。修正しないと、起動中はそもそも減衰しない(Formulas F0)。

**R6. Bond(絆)**
- 内部値 0–1000。UI に数値やゲージは出さない。
- その日の手動操作(最初に開く、世話、フォーカス完走)で増え、日次上限がある(F1)。
- 3 日以上来ないと少しずつ減る。ただし到達済みのティア下限は割らない(P3: 解放したものは失わない)。
- ティア(100 / 300 / 600 / 1000)ごとに台詞プール・表情の種類・MEMORIES の枠が広がる。
- Bond は通貨ではない。コインで買えず、コインにも換金できない。

**R7. MEMORIES**(Standard)
- 条件を満たすと、日付付きの 1 行が解放される(例: `DAY 1 — ALL GROWN UP`、`DAY 14 — STAYED UP LATE`、`DAY 30 — STILL HERE`、`FIRST BLACKJACK`)。
- 実績パネル(`frontend/js/main.js:247-277`)に `MEMORIES` タブを追加して表示する。テキストのみで、追加のアートは不要。
- ペットごとに記録し、死亡・復活後は「アルバム」として読み取り専用で残す。
- 初期セットは 12 本: 時刻系 3、継続日数系 4、ミニゲーム系 2、季節系 3。

**R8. フォーカス・バディ**(Standard)
- ポップオーバーの `FOCUS` で 25 分のセッションを開始する。dot は `calm` を維持し、トレイの「話したい」表情は出さない。
- 25 分完走で Bond +15。上限は 1 日 2 回。**コインは付与しない**(A7、経済の判断)。
- 5 分未満でやめた場合は記録しない。ペナルティもない。

**R9. トレイ表情**(Standard、ux-designer の L1 層)
- 白黒のテンプレート画像で、静止フレーム 4 種(通常 / 眠そう / ご機嫌 / 話したい=右上に 1px ドット)。連続アニメーションにはしない。
- 差し替えは状態が変わったときだけで、最大でも 1 分に 1 回。「話したい」は F3 の予算内でのみ出し、ポップオーバーを開いたら消える。
- tooltip を状態に合わせて更新する(例: `dot — sleepy`)。VoiceOver にも同じ内容が読まれる。

**R10. 死と別れ**
- 死亡条件は変えない(`lifecycle.rs:3-19`)。演出は MVP から入れる。アルバム化は MEMORIES と同じく Standard から。大人期の減衰が 0.4 倍なので、死に至るには約 42 時間の完全放置が必要になる(F4。現行 Child は約 17 時間)。
- 死亡が Adult で起きた場合は、既存の `GAME OVER` 表示(`frontend/index.html:77-84`)を大人用の演出に置き換える。dot が目を閉じ、`THANK YOU.` を 1 行出し、ボタンは `NEW FRIEND?` にする。放置を責める文言は使わない。
- 既存どおり、コイン・実績・ストリークは残る(`stats.rs:20`)。Bond は 0 に戻り、MEMORIES はアルバムへ移る。

**R11. 禁止事項**(制約)
- ユーザーは Bond を直接操作できない。Companion 機能はコインを生まない。
- Companion は世話の代わりをしない(dot が自分で食事をとることはない)。
- 静寂時間・QUIET 中・フォーカス中は、トレイの「話したい」表情を出さない(`concerned` の安全弁も静寂時間中は出さない)。

### States and Transitions

**主状態**

| 状態 | 説明 | 遷移先(条件) |
|---|---|---|
| `Inactive` | Egg/Baby/Child、または死亡中 | `Onboarding`(Adult 到達後に初めて開く) |
| `Onboarding` | 吹き出しで同意を確認中 | `ActiveBasic`(LATER/無視)/ `ActiveFull`(YES) |
| `ActiveBasic` | MVP の範囲: ポップオーバー内の Mood と台詞、Bond の蓄積 | `ActiveFull`(設定で ON)/ `Onboarding`(LATER から LATER_REPROMPT_D=7 日後の最初のオープン。1 回だけ)/ `Farewell` |
| `ActiveFull` | Standard の範囲: 離席検知、トレイ表情、フォーカス | `ActiveBasic`(設定で OFF)/ `Farewell` |
| `Farewell` | Adult で死亡 | `Inactive`(NEW FRIEND? で Egg に戻る) |

**直交するフラグ**(主状態とは独立に保持する。主状態を書き換えないので、「元の状態に戻す」処理は不要)

| フラグ | 立つ条件 | 効果 |
|---|---|---|
| `focus` | `ActiveFull` で FOCUS を開始(最大 25 分) | Mood を `calm` に固定し、トレイの「話したい」を止める。完走・中止・死亡で下ろす |
| `muted` | マスター OFF、または `QUIET 1H` / `QUIET TODAY` が有効 | 吹き出し・トレイの表出を止める。世話ループ・Bond・focus の計時は続く |
| `quiet_hours` | 現在時刻が静寂時間 S–E の中 | トレイの「話したい」を止める。ポップオーバー内の台詞は出す(ユーザーが自分で開いたため) |

- focus と muted は同時に立ちうる。その場合はフォーカスの計時は続き、表出はすべて止まる。

### Interactions with Other Systems

| 相手 | 入力(→Companion) | 出力(Companion→) | インターフェースの所有者 |
|---|---|---|---|
| Pet State / Needs(`state.rs`, `needs.rs`) | stage, needs, activity, is_alive | 大人期の減衰倍率 | Needs が倍率を受け取る(Needs 側が所有) |
| Evolution(`evolution.rs`) | Adult 到達 | なし | Evolution |
| Lifecycle(`lifecycle.rs`) | 死亡、revive | Farewell 演出、Bond のリセット、アルバム化 | Lifecycle が通知、Companion が処理 |
| GameStats / Economy(`stats.rs`, `economy.rs`) | 世話・ゲームのイベント(コマンド成功時に Companion へ通知)。GameStats は累計しか持たない(`stats.rs:5-27`)ので、当日カウンタ n_care / n_focus と「その時点で need < 80 か」の判定は Companion が `companion.json` に持つ | **コインの増減なし**(MVP)。Standard では見た目アイテムのシンク案あり(決定事項 4) | Economy |
| Achievements(`achievements.rs`) | なし | 新しい実績候補 2 件。`Old Friends` は大人期に一緒にいた日数(手動オープンした日)が 30 日。`Memory Keeper` は MEMORIES 10 件(Standard)。Bond のティアは実績にしない(A3: 見せない絆が露出するため)。`check_achievements(stats, pet)`(`achievements.rs:87`)は Companion の状態を受け取れないので、引数に `&CompanionState` を追加するインターフェース変更が必要 | Achievements |
| Storage(`storage/mod.rs`) | 読み込み | `companion.json` の保存 | Storage |
| Renderer / UI(`renderer.js`, `main.js`) | なし | Mood ごとの目の描画、吹き出し、MEMORIES タブ | UI |
| Tray(`main.rs:30-61`) | クリック | アイコンと tooltip の差し替え | Tray |
| OS シグナル(Standard) | ローカル時刻、アイドル秒数、電源状態 | なし | Companion の signals モジュール |

---

## Formulas

### F0. 前提: 現行減衰の切り捨て問題(既存バグ、要修正)

`decay_step = floor(rate × Δt_min)`(`needs.rs:22,27,33,37`。`as u8` による切り捨て)

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| rate | float | 0.1–0.2 /分 | constant(`needs.rs:4-6`) | 減衰率 |
| Δt_min | float | 0–∞ | 計算(`state.rs:75`) | 前回の update からの経過分 |

- **出力**: フロントは 5 秒ごとにポーリングする(`main.js:70-73`)ので Δt_min ≈ 0.083。hunger では 0.2 × 0.083 = 0.017 → 0 になる。update のたびに `last_update = now` で端数が捨てられる(`state.rs:97`)ため、**起動中はほぼ減衰しない**。アプリを閉じている間の分だけが、次回起動時にまとめて減る。
- **修正方針(P2)**:
  1. needs を f32 で持つか、端数を蓄積して捨てないようにする。
  2. 経過時間は `Δt = clamp(now − last_update, 0, OFFLINE_CAP)` とする。OFFLINE_CAP は 72 時間で、needs と age の両方に適用する。時計が過去に戻った場合(負の値)は 0 にする。
  3. **長い Δt は 1 分刻みで逐次積分する**(最大 72×60 = 4320 ステップ。起動時に 1 回だけ走る軽い処理)。各ステップで health の増減と死亡を判定する。睡眠が終わる時刻をまたぐ場合は、その時刻で区間を分ける。
     - 一括計算のままだと、health の判定が区間の終わりの状態で 1 回しか行われない(`needs.rs:41-47`)。そのため F4 の死亡時刻が約 33 時間に縮み、睡眠の途中終了も「最初から起きていた」扱いになる(`state.rs:81-89`)。
- 時計を巻き戻すと `(now − last_update) as u64` が巨大な値になる(`state.rs:75`)。この問題も同じ修正で塞ぐ。
- 補足: `needs.rs:34` の u8 加算は、睡眠が最長 4 時間で、減衰計算の前に起床処理が走る(`state.rs:81-86`)ため、現状ではオーバーフローに到達しない。逐次積分にすれば構造的にも起きない。

### F1. Bond の日次確定

**確定のタイミング**: その日最初の update(ローカル日付 ≠ `last_settle_date`)で 1 回だけ確定する。アプリが起動していなかった日があっても、ループは回さない。

`G = min(G_CAP, G_GREET·g + G_CARE·min(n_care, N_CARE_MAX) + G_FOCUS·min(n_focus, N_FOCUS_MAX))`(前回の確定以降に貯まった当日カウンタ。確定後に 0 に戻す)
`overdue(today) = min(max(0, m − GRACE), M_MAX)`
`D = DECAY · max(0, overdue(today) − overdue_applied)`(差分方式。確定後に `overdue_applied ← overdue(today)`。手動で開いたら `overdue_applied ← 0`)
`B' = clamp(max(tier_floor(B), B + G − D), 0, 1000)`

- 差分方式にしたので、毎日起動していても、閉じていて一度だけ確定しても、減衰の合計は同じになる。

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| B | int | 0–1000 | save(`companion.json`) | 現在の Bond |
| g | bool | 0/1 | 当日カウンタ | その日に手動で開いたか(トレイクリックのみ数える) |
| n_care | int | 0–∞ | 当日カウンタ | 対象の need が 80 未満のときに行った FEED/PLAY/SLEEP の回数 |
| n_focus | int | 0–∞ | 当日カウンタ | 完走したフォーカス数 |
| m | int | 0–∞ | 日付差 | 最後に手動で開いたローカル日付から、今日までの日数(翌日に開いたら m=1) |
| G_GREET | int | 10 | data file | 挨拶の加点 |
| G_CARE | int | 3 | data file | 世話 1 回の加点 |
| N_CARE_MAX | int | 5 | data file | 世話の加点の日次上限回数 |
| G_FOCUS | int | 15 | data file | フォーカス 1 回の加点 |
| N_FOCUS_MAX | int | 2 | data file | フォーカスの日次上限回数 |
| G_CAP | int | 50 | data file | 1 日の加点上限 |
| overdue_applied | int | 0–30 | save | すでに減衰として適用した超過日数 |
| GRACE | int | 2 | data file | 減衰しない猶予日数 |
| DECAY | int | 8 | data file | 1 日あたりの減衰 |
| M_MAX | int | 30 | constant | 減衰を計算する日数の上限 |
| tier_floor | int | 0/100/300/600/1000 | data file | 到達済みティアの下限 |

- **出力範囲**: G は 0–50/日、B は 0–1000。
- **ティア到達日数**(100 / 300 / 600 / 1000)

  | 段階 | 1 日の加点 | 到達日数 |
  |---|---|---|
  | MVP 最速(Focus なし: 10+15) | 25 | 4 / 12 / 24 / 40 |
  | MVP 標準(挨拶 + 世話 2 回) | 16 | 7 / 19 / 38 / 63 |
  | Standard 最速 | 50 | 2 / 6 / 12 / 20 |
  | Standard 標準(16 + 1 日おきに Focus 1 回で約 +7.5) | 約 24 | 5 / 13 / 25 / 42 |

  - 根拠: CLAUDE.md の「旅は意味のあるものに」(CLAUDE.md:22-28)に合わせ、Standard の標準的な使い方で最上位まで約 1.5 か月かかるようにした。
  - MVP では最上位まで約 2 か月かかるが、MVP のティアが解放するのは台詞プールだけなので許容する。
- **例**: B=280 で、最後に開いた日(d0)の分はすでに確定済み。d1–d3 は開かず、アプリは起動したまま。
  - d3 の確定: overdue=1 なので D=8、B=272。
  - d4 の確定: overdue=2、差分 1 なので D=8、B=264(下限 100 は割らない)。アプリを閉じていて d4 に一度だけ確定した場合も、D=16 で同じ 264 になる。
  - d4 に挨拶・FEED 1 回(hunger < 80)・フォーカス 1 回をすると、d5 の確定で G=28、B=292 になる。
- 日付が過去に戻った場合は確定しない。

### F2. Mood の選択
重み付きの式ではなく、Core Rules R2 の優先順位表を使う(デバッグと説明がしやすいため)。

`mood = first_match(R2_table, needs_min, health, activity, hour_local, hours_since_open, focus)`

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| needs_min | int | 0–100 | 計算 | min(hunger, happiness, energy) |
| health | int | 0–100 | PetState | 体力 |
| hour_local | int | 0–23 | OS 時計(ローカル) | 現在の時(時刻) |
| hours_since_open | float | 0–∞ | 計算 | 最後に手動で開いてからの経過時間 |
| MOOD_MIN_DWELL | sec | 600 | data file | 表示中・トレイ更新時に、Mood が最低限続く時間(ちらつき防止) |
| CONCERNED_ENTER | int | needs 20 / health 30 | data file | `concerned` に入る閾値 |
| CONCERNED_EXIT | int | 30 | data file | `concerned` を解除する閾値(全 needs ≥ 30 かつ health ≥ 30。ヒステリシス) |
| SLEEPY_ENERGY | int | 35 | data file | `sleepy` になる energy の閾値 |
| SLEEPY_HOURS | hour | 23–06 | data file | `sleepy` になる時間帯 |
| CALM_NEEDS | int | 60 | data file | `calm` と `cheerful` の境界 |

- **出力**: 5 つの Mood のいずれか 1 つ。
- **DWELL の適用範囲**: ポップオーバーを開いた瞬間は、DWELL を無視して毎回計算し直す。DWELL が効くのは、開いたまま状態が変わったときと、トレイの差し替えだけ。`concerned` への遷移は DWELL を無視して即時に行う。`missed_you` は開いた瞬間にしか評価しないので、DWELL の対象外。
- **例**: 23:30、needs が 70/80/40 のとき。優先 1 は不成立、優先 2 は時刻 23–06 に該当するので `sleepy` になる。

### F3. 表出の予算(トレイの「話したい」表情)

`tokens' = min(C, tokens + max(0, now − t_last)/R)`
`allow = tokens ≥ 1 ∧ (now − t_last_surface) ≥ GAP ∧ surfaced_today < D_MAX ∧ ¬quiet(h) ∧ ¬muted ∧ ¬focus`
`quiet(h) = (S > E) ? (h ≥ S ∨ h < E) : (S ≤ h < E)`(S == E の場合は静寂時間なし)

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| C | int | 3 | data file | バケット容量 |
| R | sec | 14400 | data file | 1 トークンの回復時間(4 時間) |
| GAP | min | 45 | data file | 最小間隔 |
| D_MAX | int | 5 | data file | 1 日の上限(ユーザー設定では増やせない) |
| S, E | hour | 22, 8 | 設定 | 静寂時間の開始と終了 |
| IDLE_AWAY_MIN | min | 10 | data file | 「話したい」のきっかけになる離席の長さ(Standard) |

- 「話したい」のきっかけは、IDLE_AWAY_MIN 以上の離席からの復帰と、`concerned` への遷移の 2 つだけ。

- **出力**: 0〜5 回/日。08:00–22:00 の 14 時間に最大 5 回なので、平均で約 2.8 時間に 1 回を超えない。
- **concerned の安全弁**: 同じ need について 6 時間に 1 回まで、トークンが 0 でも出せる。ただし静寂時間・QUIET・D_MAX が優先する。
- **例**: 09:00 にトークン 3 で表出してトークン 2。09:30 は GAP 未満なので出さない。10:00 は出せる。
- **根拠**: CLAUDE.md の「ambient, not distracting」(CLAUDE.md:10-12)。ux-designer 案(6 回/日)と systems-designer 案(5 回/日)のうち、保守的な方を採った。

### F4. 大人期の減衰と死までの時間

`rate_adult = rate_base × ADULT_DECAY_MULT`
`T_death = min(T_allzero, T_health)`
`T_allzero = max_i(100 / rate_adult_i)`(hunger・happiness・energy の 3 つがすべて 0 になる時刻: `lifecycle.rs:15`)
`T_health = (100 − 10)/rate_adult_hunger + 100/HEALTH_DECAY`(hunger が 10 を下回るまで + health が 0 になるまで: `lifecycle.rs:9`, `needs.rs:41-43`)
(放置開始時は全ステータス 100、睡眠していないと仮定)

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| rate_base | float | hunger 0.2 / happy 0.15 / energy 0.1 (/分) | constant(`needs.rs:4-6`) | 現行の減衰率 |
| ADULT_DECAY_MULT | float | 0.4(安全範囲 0.25–0.6) | data file | 大人期の倍率 |
| HEALTH_DECAY | float | 0.05 /分 | constant(`needs.rs:42`) | 危険状態での health 減少 |

- **出力**: 大人の減衰は hunger 0.08・happiness 0.06・energy 0.04 /分。
  - T_allzero = 100/0.04 = 2500 分 ≈ **41.7 時間**(energy が最後に 0 になる)。
  - T_health = 18.75 時間 + 33.3 時間 ≈ 52 時間。
  - よって **約 42 時間(1.7 日)の完全放置で死亡**する。
  - 現行 Child は T_allzero = 100/0.1 = 1000 分 ≈ 16.7 時間。
  - 2 日以上にしたい場合は ADULT_DECAY_MULT = 0.3 にする(T_allzero ≈ 55.6 時間)。
- 世話の頻度の目安: 起きている 10 時間で hunger は 100 → 52 になる。FEED(+30)を 1〜2 回すれば足りる。
- 前提: F0 の修正後(端数の蓄積 + 1 分刻みの逐次積分)の値。OFFLINE_CAP=72 時間は T_death より長いので、閉じている間も死は起こりうる。

---

## Edge Cases

- **既存セーブの pet がすでに Adult の場合**: 初回起動時に `companion.json` がなければ、B=0、`last_settle_date=今日`、`last_open_date=今日`、`overdue_applied=0` で作成する(猶予は通常の GRACE がそのまま効く)。次に開いたときに Onboarding を出す。遡ってのボーナスは付けない。
- **アプリを閉じている間に Adult に到達した場合**: 次の update で進化し、`companion_since=now` とする。最初に開いたときが Onboarding になる。
- **Companion 有効中に死亡した場合**: Farewell に遷移する。進行中のフォーカスは中止して報酬なし。Bond は確定済みの値のままアルバムに記録し、新しいペットは B=0 から始める。
- **フォーカス中にペットが死亡、またはスリープ/蓋閉じした場合**: 死亡なら即中止する。スリープ(壁時計と単調時計の差が SLEEP_DETECT_S=120 秒超)なら一時停止し、復帰後 FOCUS_RESUME_MIN=10 分以内に再開しなければ中止する(記録なし)。
- **時計を過去に戻した場合**: Δt は 0 として扱い、`last_update=now` にする。日付が `last_settle_date` より前なら、日次確定も当日カウンタのリセットもしない。
- **時計を未来へ大きく進めた場合**: Δt は OFFLINE_CAP(72 時間)で止める。needs と age の両方に適用する。日次確定は 1 回だけで、overdue は最大 30。72 時間を超えて本当に閉じていた場合も同じ扱いになり、長期不在のあとの成長は 72 時間分で止まる(副作用として許容する)。
- **タイムゾーン変更・DST の場合**: 日付境界はローカル日付で判定し、日付が変わったときに 1 回だけ確定する。現行のストリーク(`stats.rs:117`)は UTC 日付で判定しているため、Companion の日付と食い違いうる。ストリークも将来ローカル日付に揃えるべきで、Open Questions に記載した。
- **数日間アプリが起動していない場合**: 次回起動時に F0 修正後の減衰を一度に適用する(F4 により約 42 時間以内なら生存)。Bond は F1 で m から一括計算し、日数分のループはしない。
- **静寂時間が日をまたぐ場合**: F3 の `S > E` 側の式で判定する。S == E は静寂時間なしとする。
- **蓋閉じ・スリープ中のアイドル値**: スリープ中の値は使わない。復帰直後の 60 秒はアクティブ扱いにし、`return_after_away` の誤発火を防ぐ。
- **2 つの表出条件が同時に成立した場合**: ポップオーバー内の台詞は R3 の優先順位表で 1 つだけ選ぶ。トレイの「話したい」はきっかけを区別せず 1 回として数え、F3 の予算を 1 つだけ消費する。
- **ポップオーバーを短時間に何度も開閉した場合**: 台詞は 1 回開くごとに最大 1 行。同じカテゴリは 24 時間クールダウン。ただし Mood の表情は毎回表示する。
- **`companion.json` が壊れている場合**: 退避(`companion.corrupt-<ts>.json`)してからデフォルトで作り直す。PetState と GameStats には影響させない。前提タスク P1 と同じ方針。
- **新しいバージョンで Mood の enum を増やし、旧バージョンに戻した場合**: 未知の値は `#[serde(other)]` で `calm` として読む(パース失敗によるリセットを防ぐ)。
- **ユーザーが Companion を OFF にした場合**: Bond の蓄積と大人期の減衰倍率は続く(OFF は表現だけを止める)。ON に戻したときに罰はない。
- **退化戦略: アプリを放置して Bond を稼ぐ**: 加点は手動操作だけ。起動しているだけ・在席しているだけでは増えない。
- **退化戦略: 世話の連打**: need が 80 未満のときだけ数え、1 日 5 回まで。
- **退化戦略: 時計をいじって日次をリセットする**: 日付は前にしか進めない(`last_settle_date` は単調)。
- **退化戦略: フォーカスの開始と中止の繰り返し**: 25 分完走のみ有効で、1 日 2 回まで。コインは出ないので経済への影響はない。
- **退化戦略: 死と復活の繰り返し**: 新しいペットは B=0 からなので、得はない。

## Dependencies

| 依存先 | 方向 | 種類 | インターフェース | 状態 |
|---|---|---|---|---|
| Pet State / Needs | 上流 | Hard | `PetState` の読み取り、大人期の減衰倍率の注入 | 実装済み(GDD なし)。**P2 の修正が前提** |
| Evolution | 上流 | Hard | `stage == Adult` | 実装済み。CLAUDE.md の「日単位への延長」TODO が入ると、Adult 到達は約 1.5〜2 週間後になる |
| Lifecycle | 上流 | Hard | 死亡と revive のフック | 実装済み |
| Storage | 上流 | Hard | `companion.json` の読み書き | 実装済み。**P1(原子的な書き込み、壊れたファイルを黙って作り直さない)が前提** |
| GameStats | 上流 | Soft | 当日の世話回数、ゲームの結果(MEMORIES 用) | 実装済み |
| Economy | 双方向(Soft) | Soft | MVP では接続しない。Standard ではシンク案(決定事項 4) | 実装済み。**P4(ゲーム報酬の検証と日次上限)は経済側の前提** |
| Achievements | 下流 | Soft | 実績 2 件を追加 | 実装済み |
| Renderer / UI / Tray | 下流 | Hard | Mood の描画、吹き出し、トレイ画像 | 実装済み。**P3(非表示中の描画ループ停止)が前提** |
| OS シグナル(Standard) | 上流 | Soft | ローカル時刻、HID アイドル、IOPowerSources | 未実装 |

- 双方向の整合: 既存システムには GDD がない。各実装の PR で、逆方向の依存(例: Needs が大人期の倍率を受け取ること)をコードコメントかテストで明示する。

### 前提タスク(Companion の実装より先に行う)
| ID | 内容 | 根拠 | 重要度 |
|---|---|---|---|
| P1 | セーブを原子的に書く(tmp → fsync → rename)。`.bak` を残す。パースに失敗したら退避し、黙って新しいペットにしない | `lib.rs:16-26` が失敗時に `PetState::new()` を返し、次の保存で上書きして永久に失われる。`storage/mod.rs:29` の `fs::write` は非原子的 | 高 |
| P2 | needs の端数を蓄積する。Δt を [0, OFFLINE_CAP] に clamp する。長い Δt は 1 分刻みで逐次積分する | F0 | 高 |
| P3 | ポップオーバーが非表示の間は rAF ループと 5 秒ポーリングを止め、Rust 側の 60 秒 tick に集約する。パッシブ収入の `accrue_passive` もポーリング経由で呼ばれている(`commands.rs:61`)ので、tick 側へ移す(そうしないと非表示中にコインが貯まらなくなる) | `renderer.js:61` に停止条件がない。`main.js:70` が常時動いている | 中 |
| P4 | ゲーム報酬をサーバー側で検証する(開始トークン、最短プレイ時間)。日次上限 30 枚 | `commands.rs:191-203` がクライアントの申告をそのまま信じている。devtools から 1 回最大 10 枚(memory: `economy.rs:39-43`)を無限に稼げる | 中(経済) |
| P5 | README の保存先の記述を修正する(実際は `~/Library/Application Support/dot/`) | `dirs::config_dir()`(`storage/mod.rs:7`)と README.md:79 が食い違う | 低 |

## Tuning Knobs

| Knob | 初期値 | 安全範囲 | 上げすぎると | 下げすぎると | 根拠 |
|---|---|---|---|---|---|
| ADULT_DECAY_MULT | 0.4 | 0.25–0.6 | 世話が義務に戻る | 世話の意味が消え、死もほぼ起きない | systems 0.4 / CD 1/3 の中間。死まで約 42 時間(0.3 なら約 56 時間) |
| G_GREET | 10 | 5–15 | 開くだけで育つ | 毎日開く動機が弱まる | 日次上限の 1/5 |
| G_CARE / N_CARE_MAX | 3 / 5 | 2–5 / 3–8 | 世話の連打が有利になる | 世話と絆のつながりが見えない | 最大 15 で上限の 30% |
| G_FOCUS / N_FOCUS_MAX | 15 / 2 | 10–20 / 1–3 | 生産性アプリ化する(アンチピラー 2) | フォーカスの存在意義が薄れる | 加点の主役にしない |
| G_CAP | 50 | 30–80 | ティアがすぐ尽きる | 最上位まで 2 か月を超える | 最上位まで最速 20 日、標準 40 日 |
| GRACE / DECAY | 2 日 / 8 | 1–5 / 0–15 | (GRACE を上げると)放置が無害化する | (DECAY を上げると)罪悪感を生む | ティア下限で守るので穏やかに |
| Bond ティア | 100/300/600/1000 | — | — | — | 標準ペースで 4/12/24/40 日 |
| C / R / GAP / D_MAX | 3 / 4h / 45 分 / 5 | 2–4 / 2–6h / 30–90 分 / 3–6 | 邪魔になる | 存在が感じられない | F3 |
| 静寂時間 S–E | 22–08 | ユーザー設定 | — | — | ux-designer 案 |
| MOOD_MIN_DWELL | 600 秒 | 300–1800 | 反応が鈍い | 表情がちらつく | systems 案 |
| AWAY_RETURN_H / MISSED_YOU_H | 4 時間 / 24 時間 | 2–8 / 12–48 | 挨拶が稀になる | 毎回 `WELCOME BACK` になる | 業務日の昼休みを越える程度 |
| IDLE_AWAY_MIN(Standard) | 10 分 | 5–30 | 復帰の検知が遅れる | 席を立つたびに反応する | ux 案 |
| SIGNAL_TICK(Standard) | 60 秒 | 30–120 | 反応が遅れる | CPU を使う | TD 案。非表示中の CPU を 0.1% 未満にする |
| TRAY_SWAP_MIN | 60 秒 | 30–300 | — | ちらつく | TD 案 |
| FOCUS_LEN | 25 分 | 15–50 | — | — | ポモドーロ慣行 |
| LINE_MAX_CHARS | 24 | 16–30 | 吹き出しに収まらない | 意味が伝わらない | 280px 幅、pixel 書体 |
| BUBBLE_TTL | 6 秒 | 4–10 | 画面に残りすぎる | 読み切れない | ux 案 |
| LINE_SPEAK_P | 0.5 | 0.2–0.8 | おしゃべりになる | 存在感が消える | 「沈黙も表現」(Game Feel) |
| LINE_HISTORY / LINE_CATEGORY_COOLDOWN | 20 件 / 24 時間 | 10–40 / 12–48 | (下げると)同じ台詞が目立つ | — | ux 案 |
| OFFLINE_CAP | 72 時間 | 48–168 | 時計を進めたときの影響が大きい | T_death(42 時間)を下回ると、閉じている間は死ななくなる | Review 1 #1。T_death より長くする必要がある |
| CONCERNED_ENTER / EXIT | 20(health 30)/ 30 | 10–30 / ENTER+5 以上 | 心配性になる | 危険に気づけない | systems 案。既存の status `hungry` が < 20(`state.rs:208`)なのに合わせた |
| SLEEPY_ENERGY / SLEEPY_HOURS | 35 / 23–06 | 20–50 / — | — | — | systems 案 |
| CALM_NEEDS | 60 | 40–80 | `cheerful` が稀になる | 常に `cheerful` になる | systems 案 |
| CONCERNED_VALVE_H | 6 時間 | 3–12 | — | 心配の表出が多すぎる | F3 の安全弁 |
| LATER_REPROMPT_D | 7 日 | 3–14 | 機能に気づかれない | しつこい | ux 案。再提示は 1 回だけ |
| BATTERY_LOW_PCT / REARM_PCT | 20% / 50% | 10–30 / 40–80 | うるさい | 間に合わない | ux 案 |
| SLEEP_DETECT_S / FOCUS_RESUME_MIN | 120 秒 / 10 分 | 60–300 / 5–30 | — | — | systems 案 |

- すべて `companion` 用のデータファイル(または Rust の定数モジュール)にまとめる。コード中にマジックナンバーとして散らさない。

## Visual/Audio Requirements

- **目の表情 5 種**(Mood ごと)を既存の Adult スプライト(`renderer.js:241-285`)の目の部分の差し替えで表現する。体のピクセルは変えない(P2 少ないもので伝える)。
- **トレイの静止画 4 種**を 16×16 / 32×32 の @2x テンプレート画像(白黒、`icon_as_template(true)`)で用意する。現行のトレイは通常のアプリアイコンを流用しており、テンプレート指定がない(`main.rs:31`)。
- **Ambitious で検討**: ネイティブ通知を有効にするなら、OS 標準の音だけを使う。
- アニメーションは CLAUDE.md の「控えめに」(CLAUDE.md:8-12)に従う。表情を切り替えるときに 1 回瞬きする程度に留める。
- 音: なし(アンチピラー 1)。

## Game Feel

- 開いた瞬間(100ms 以内)に、こちらを向いた目と 1 行が見える。待たせない。
- 言葉は少なく、毎回は話さない。1 回開いて何も言わないことも多い。沈黙も表現として扱う。
- `prefers-reduced-motion` が有効なら、瞬きと揺れを止めて静止画にする。

## UI Requirements

- 吹き出し: キャンバス(128×128、`index.html:24`)の上に重ねる。最大 2 行・24 文字。`role="status"` / `aria-live="polite"` を付ける。VoiceOver が有効な間は自動で消さない。
- 設定: `*` メニューに `COMPANION` サブメニューを追加する(トグルは `<button aria-pressed>`)。設定の永続化は `companion.json` に含める(tauri-plugin-store は使わず、保存方式を 1 つに保つ)。
- `FOCUS` ボタン(Standard): 情報行(`index.html:61-69`)の status の位置に `FOCUS 18m` と残り時間を表示する。
- `MEMORIES` タブ(Standard): 実績オーバーレイ内に置く。
- 状態の情報行: Adult で `activity == Idle` のときだけ、status の `content/hungry/tired/sad` を Mood 名に置き換える。`sleeping (Xh Ym left)` / `eating` / `playing`(`state.rs:188-218`)と `FOCUS 18m` は、Mood より優先して表示する。

## Cross-References

- CLAUDE.md:31-41(Companion の TODO)、CLAUDE.md:15-28(進化を日単位に延ばす TODO。Dependencies を参照)、CLAUDE.md:44-49(Design Principles)
- 経済: `backend/src/pet/economy.rs`、`backend/src/pet/stats.rs:29-95`
- 本 GDD のレビュー: `design/gdd/reviews/companion-review-log.md`

---

## 導入計画: 3 案と推奨

| 案 | 範囲 | 権限 | 経済 | 工数感 | 評価 |
|---|---|---|---|---|---|
| **最小(MVP)** | 前提タスク P1–P3、R1(オンボーディング)・R2(Mood)・R3(台詞、ポップオーバー内のみ)・R4(開いた時刻による不在判定)・R5(大人期の減衰)・R6(Bond の内部蓄積、ティアは台詞プールのみ解放)・R10(Farewell 演出。アルバムなし)、設定(マスター/静寂時間/QUIET) | なし(時刻のみ) | 接続なし | 小 | Adult に意味が生まれる。邪魔になるリスクはゼロ |
| **標準(推奨の到達点)** | MVP + R4 の離席検知(HID アイドル、権限不要)・R7 MEMORIES・R8 フォーカス・R9 トレイ表情・バッテリー低下の一言(非充電で 20% 未満、ポップオーバー/トレイのみ)、P4(ゲーム報酬の検証) | なし(TCC のプロンプトは 0 回) | 新しい稼ぎ口なし。見た目アイテムのシンクは決定事項 4 次第 | 中 | 閉じている間も「そばにいる」感じが出る |
| **野心的** | 標準 + オプトインのネイティブ通知(1 日 3 回まで)、オプトインのカレンダー(EventKit、予定名は表示しない)、カメラ/マイク使用中の静音、季節の MEMORIES | 通知の許可、カレンダー(TCC) | 季節アイテム(ライブ運用) | 大 | 未署名の配布ではリリースごとに TCC の許可が外れる(TD)。P1 のピラーと緊張関係にある |

**推奨**: 標準を到達点とし、**MVP → 標準の 2 段階で出す**。最初のリリースは MVP(前提タスク込み)で、そこで「開いたときの一言」が鬱陶しくないかを確かめてから、トレイ表情を足す。野心的な案は、署名と公証付きの配布に移るまで凍結する。

### プライバシーと macOS の権限
| シグナル | API | TCC | 採否 |
|---|---|---|---|
| ローカル時刻 | `chrono::Local` | 不要 | MVP |
| アイドル秒数 | `CGEventSourceSecondsSinceLastEventType` / IOKit `HIDIdleTime` | 不要 | 標準 |
| 電源/バッテリー | `IOPSCopyPowerSourcesInfo`(イベント駆動) | 不要 | 標準 |
| ロック/スリープ/復帰 | tick 間の壁時計のジャンプで近似 → 後から `NSWorkspace` の通知に置き換え | 不要 | 標準 |
| 打鍵 | `CGEventTap` / `NSEvent` のグローバル監視 | Input Monitoring / Accessibility | **採用しない** |
| カレンダー | EventKit | Calendars | 野心的(オプトイン) |
| 最前面のアプリ / 会議 | `NSWorkspace` / CoreMediaIO | 不要(ウィンドウタイトルは Screen Recording が必要) | 野心的(静音の用途に限る) |
| Focus/おやすみモード | `INFocusStatusCenter`(entitlement が必要) | — | **採用しない**(手動の QUIET で代替) |
| ネイティブ通知 | tauri-plugin-notification | 通知の許可 | 野心的(オプトイン、未署名では動作検証が必須) |

- シグナルの値は揮発させ、保存しない。保存するのは設定・Bond・MEMORIES だけ。
- **App Review 相当のリスク**: 現在は未署名で DMG/Homebrew 配布(README.md:21-37)。MAS に出すなら App Sandbox が必須で、打鍵監視はほぼ確実に却下される。カレンダーは用途の説明が必要。採用したシグナル(時刻・アイドル・電源)は Sandbox 下でも使える。

### 軽量性の予算
| 状態 | 平均 CPU | 起床 |
|---|---|---|
| 非表示 | < 0.1% | 1 分に 1 回以下(Rust の tick のみ。フロントのタイマーは 0 本) |
| 表示中 | < 2% | 描画は 10〜15fps 上限で足りる(スプライトの更新間隔は 200ms: `renderer.js:12`) |

### セーブ互換
- `~/Library/Application Support/dot/companion.json` を別ファイルにし、`schema_version: 1` を持たせる。既存の `pet-state.json` と `game-stats.json` は変更しない。
- 新しいフィールドはすべて `#[serde(default)]` にし、enum には `#[serde(other)]` を付ける。
- `companion.json` がない場合は自動で作る(Edge Cases の「既存セーブの pet がすでに Adult の場合」)。

---

## Acceptance Criteria

- **GIVEN** pet が Child で age が閾値の直前、**WHEN** 閾値を越えて最初にポップオーバーを開く、**THEN** 吹き出しに `STAY WITH YOU?` と `[YES][LATER]` が表示され、モーダルは出ない。(R1)
- **GIVEN** Onboarding で何も押さずに閉じた、**WHEN** 再び開く、**THEN** ActiveBasic で動き、トレイのアイコンは変わらない。(R1)
- **GIVEN** Adult で needs が 15/80/80、**WHEN** 開く、**THEN** 目は `concerned` の表情になり、status に `concerned` と表示される。(R2/F2)
- **GIVEN** 23:30、needs がすべて 70 以上、**WHEN** 開く、**THEN** Mood は `sleepy`。(R2/F2)
- **GIVEN** その日すでに 1 回開いており(`greet_first_today` は消化済み)、needs はすべて 60 以上、前回開いてから 5 時間後、**WHEN** 開く、**THEN** `return_after_away` カテゴリの台詞が 1 行表示される。さらに 4 時間後に開いても、同じカテゴリの台詞は出ない。(R3/R4)
- **GIVEN** その日最初のオープンで、前回開いてから 30 時間後、**WHEN** 開く、**THEN** 表示されるのは `greet_first_today` の 1 行だけ(優先 1)。Mood は `missed_you`。(R2/R3)
- **GIVEN** 優先 1–6 のカテゴリがすべて不成立で、乱数シードを固定して LINE_SPEAK_P=0.5、**WHEN** 1000 回開く、**THEN** 発話の割合は 0.5±0.05。(R3、単体テスト)
- **GIVEN** 台詞を表示した直後、**WHEN** 6 秒間ホバーしない、**THEN** 吹き出しは消える。ホバー中は消えない。(R3)
- **GIVEN** 直近 20 件の台詞履歴、**WHEN** 100 回連続で選ぶ、**THEN** 履歴と重複する ID は 0 件。(R3、単体テスト)
- **GIVEN** P2 修正後の Adult で needs が 100、**WHEN** 600 分経過(起動中、5 秒ポーリング)、**THEN** hunger は 52±1。同じ条件の Child では 100−120 で 0 になる。(R5/F4)
- **GIVEN** P2 修正後、**WHEN** 5 秒ごとに 720 回 update する、**THEN** hunger は 1 回の 3600 秒 update と ±1 で一致する。(F0)
- **GIVEN** P2 修正後の Adult で全ステータス 100、アプリは閉じたまま、**WHEN** 41 時間後に起動する、**THEN** 生存しており、hunger と happiness は 0、energy は 1 以上。(F0 の逐次積分 / F4)
- **GIVEN** Sleeping(残り 1 時間)の状態でアプリを閉じた、**WHEN** 3 時間後に起動する、**THEN** 最初の 1 時間は睡眠中の規則(energy +0.5/分、happiness は減らない)、残りの 2 時間は起きている規則で計算した値と ±1 で一致する。(F0)
- **GIVEN** B=0 で、その日に開く・FEED 2 回(hunger < 80)・フォーカス完走 1 回、**WHEN** 日付が変わる、**THEN** B=31。(F1)
- **GIVEN** 同じ日に、FEED を hunger ≥ 80 で 3 回と hunger < 80 で 7 回行い、フォーカスを 3 回完走して開いた、**WHEN** 日付が変わる、**THEN** G=min(50, 10+15+30)=50。(F1)
- **GIVEN** B=280 で、最後に開いた日から 4 日後(m=4)、アプリは閉じていた、**WHEN** 起動して日次確定する、**THEN** B=264 であり、100 を下回らない。(F1)
- **GIVEN** 同じ条件でアプリは d1–d4 の間ずっと起動していた(毎日確定が走る)、**WHEN** d4 の確定後、**THEN** B=264(閉じていた場合と同じ値)。(F1 の差分方式)
- **GIVEN** B=105(ティア 100 に到達済み)で 30 日未訪問、**WHEN** 確定する、**THEN** B=100。(F1 のティア下限)
- **GIVEN** 時計を 2 日戻す、**WHEN** update が走る、**THEN** age・needs・B・当日カウンタはいずれも変化しない。(Edge Cases)
- **GIVEN** 静寂時間 22–08 で 23:00 に 10 分離席して戻る、**WHEN** 戻る、**THEN** トレイの「話したい」表情は出ない。(F3)
- **GIVEN** ActiveFull で、08:00 から 1 時間ごとに 15 分(IDLE_AWAY_MIN 以上)離席して復帰する、**WHEN** 22:00 になる、**THEN** 「話したい」表情は 5 回以下で、各回の間隔は 45 分以上。(F3)
- **GIVEN** ActiveFull で QUIET 1H を押した、**WHEN** 15 分の離席から復帰する、**THEN** 「話したい」表情は出ない。60 分後には再び出せる。(F3 / muted)
- **GIVEN** ActiveFull で Mood が `calm` から `cheerful` に変わる条件が 3 分ごとに入れ替わる、**WHEN** ポップオーバーを開いたまま 30 分経つ、**THEN** 表情の切り替えは 3 回以下(MOOD_MIN_DWELL=600 秒)。(F2)
- **GIVEN** ActiveFull で「話したい」表情が出ている、**WHEN** トレイをクリックして開く、**THEN** トレイは通常の表情に戻り、tooltip は `dot — <mood>` になる。(R9)
- **GIVEN** Onboarding で LATER を押した、**WHEN** 7 日後に開く、**THEN** `STAY WITH YOU?` がもう一度だけ表示される。そこでも LATER を押すと、以降は表示されない。(R1 / States)
- **GIVEN** Adult が Idle で Sleeping 中、**WHEN** 開く、**THEN** status は `sleeping (Xh Ym left)` のまま表示され、Mood 名では上書きされない。(UI)
- **GIVEN** Adult で全ステータス 100 から 43 時間の完全放置、**WHEN** 起動する、**THEN** `THANK YOU.` と `NEW FRIEND?` が表示され、コインと実績は保たれる。Standard では MEMORIES がアルバムに残る。(R10/F4)
- **GIVEN** Companion モジュールの全公開関数(Bond 確定、フォーカス完走、台詞選択、オンボーディング)、**WHEN** 単体テストで各関数の前後の `GameStats.coins` と `total_coins_earned` を比べる、**THEN** どちらも変化しない。Companion のコードから `add_coins` / `try_spend` への呼び出しは静的にも 0 件。(R11/A7)
- **GIVEN** `companion.json` に不正な JSON、**WHEN** 起動する、**THEN** `companion.corrupt-*.json` に退避して Companion だけをデフォルトで作り直す。pet-state.json と game-stats.json は変わらない。(Edge Cases/P1)
- **GIVEN** 既存の v1.0.3 の Adult のセーブ、**WHEN** 新しいバージョンで起動する、**THEN** ペットのステータスは変わらず、B=0 の companion.json が作られる。(Edge Cases)
- **GIVEN** ポップオーバーを閉じて 10 分、**WHEN** Activity Monitor で計測する、**THEN** 平均 CPU は 0.1% 未満で、TCC のダイアログは 1 回も出ていない。(P3/プライバシー)
- **GIVEN** フォーカスを 4 分で中止、**WHEN** 日付が変わる、**THEN** n_focus は 0 で、ペナルティはない。(R8)

## Open Questions

1. ストリークの日付(UTC: `stats.rs:117`)と Companion の日付(ローカル)を揃えるか。推奨: 両方ローカルに統一する(既存のストリークを移行する時に 1 日分を救済する)。
2. 進化を日単位に延ばす TODO と Companion を同じリリースで出すか。推奨: 別にする。Companion を先に出す(日単位化は既存ユーザーの Child を巻き戻すかどうかの判断が必要)。
3. P2 の修正で Baby/Child の実効的な減衰が急に強まる(起動中も減るようになる)。この再調整を Companion のブランチに含めるか。推奨: P2 を独立した PR にして先に出す。
4. 台詞のローカライズ(日本語)を行うか。推奨: 英語の大文字に統一したまま(UI 全体が英語のため)。

---

## レビュアーへの決定事項

1. **MVP の範囲** — 推奨: 「最小(MVP)」案を初回リリースにし、標準を到達点にする。理由: 前提タスク P1–P3 自体が大きく、まず邪魔にならないことを実地で確かめたい。権限ゼロ・経済の接続ゼロで、失敗しても戻せる。
2. **権限を要する連携の可否** — 推奨: 打鍵監視と Focus 検知は採用しない。カレンダーと通知は野心的な案のオプトインまで凍結する。MVP と標準は TCC のプロンプト 0 回。理由: 未署名の配布ではリリースごとに許可が外れる(TD)。キーロガーに見えるリスクがあり、ピラー P1 とアンチピラー 4 に反する。アイドル時間は権限なしで在席を推定できる。
3. **コイン経済との接続(ファウセット)** — 推奨: Companion からコインを一切出さない(フォーカスも Bond のみ)。理由: 現状ですでにカジュアルで 1 日約 +50 枚、ヘビーで約 +190 枚の余剰がある(economy-designer の試算)。活動報酬は自動クリック等で稼げてしまう。
4. **コイン経済との接続(シンク)** — 推奨: 標準の段階で、Bond ティアで解放する白黒アクセサリを最大 4 種(40/80/150/300 枚、ステータス効果なし、1 枠)だけ導入するかを、MVP の反応を見て判断する。MVP では導入しない。理由: 余剰を吸うシンクは必要(economy)。一方、コスメショップはミニマルな美学を壊す(creative)。両者の妥協として、数量を絞り、絆で解放する形にした。
5. **既存経済の穴(P4)** — 推奨: Companion とは別の PR で、ゲーム報酬のサーバー側検証と日次上限 30 枚を先に入れる。理由: クライアントの申告を信じる現状では、どんな価格設計も無意味になる。
6. **通知頻度の既定値** — 推奨: OS 通知は作らない。トレイの「話したい」表情は 1 日最大 5 回・最小間隔 45 分・静寂時間 22–08。ポップオーバー内の台詞は 1 回開くごとに最大 1 行・同じカテゴリは 24 時間に 1 回。理由: CLAUDE.md の「calm and ambient」。ux 案と systems 案のうち保守的な方を採った。
7. **大人期の死** — 推奨: 死はありうる(約 42 時間の完全放置。現行 Child の約 2.5 倍)とし、演出は「別れ」(`THANK YOU.` / `NEW FRIEND?`)にする。床値を設けて死ななくする案は採らない。理由: 緊張がゼロになると世話の意味が消える。ただし罰ではなく別れとして扱い、罪悪感を避ける。
8. **復活時の絆** — 推奨: 新しいペットは B=0 から始め、前のペットの MEMORIES はアルバムに永久保存する。理由: 新しいペットは新しい関係(CD・economy の一致した意見)。systems 案の 25% 引き継ぎは、死を軽くしすぎる。
9. **絆の可視化** — 推奨: 数値は出さず、表情・台詞・MEMORIES で間接的に伝える。理由: ゲージを見せると最適化の対象になり、P3「時間が関係を育てる」が崩れる。
10. **前提タスク P1(セーブ消失)と P2(減衰の切り捨て)の扱い** — 推奨: Companion より先に、独立した修正 PR として出す。理由: P1 はデータ消失(重大度: 高)、P2 は大人期の減衰倍率の前提になる。どちらも Companion がなくても必要な修正。

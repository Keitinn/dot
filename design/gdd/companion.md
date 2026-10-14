# Companion(大人期コンパニオン)

> **Status**: In Design — Revision 2(オーナー回答の反映: 決定事項 1–10 承認、不在時間の扱い、複数端末の同期)。Review 2(NEEDS REVISION)の指摘 13 件を反映済み。決定事項 11–17 は PR #3 のマージ(2026-10-14)で承認、決定事項 18(大人期の減衰倍率 0.2)はオーナーが明示的に承認(2026-10-14)(`design/gdd/reviews/companion-review-log.md`)
> **Author**: Keitinn + Claude(creative-director / economy-designer / ux-designer / technical-director / systems-designer 協議)
> **Last Updated**: 2026-10-14
> **Last Verified**: 2026-10-14(コード基準: commit `767df01`。backend/ と frontend/ は `3167e43` から変更なし)
> **関連 Issue**(Keitinn/dot): #16 不在時間で死なないようにする / #17 複数端末のセーブ同期 / #10 進化期間(保留)/ #11 P1 / #12 P2 / #13 P4
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
| A4 | 大人期の減衰 | 現行の減衰率の 0.2 倍(Revision 2 で 0.4 から変更。決定事項 18、承認済み 2026-10-14) | 現状のまま / 減衰なし / 0.4 倍(1 日 FEED 1 + PLAY 1 では維持できない) |
| A5 | 大人期の死 | **Revision 2 で修正**: 死は在席時間(アプリ起動中・Mac が起きている・ユーザーの入力あり)に世話が途切れたときだけ起きる。不在時間(アプリ非起動・スリープ・蓋閉じ・長い離席)では死なない(§不在時間の扱い)。演出は「別れ」 | 床値を設けて死なない / 不在時間も減衰させる(週末ごとに死ぬ) |
| A12 | 不在時間の扱い(Revision 2) | 不在時間は「深い眠り」として扱う: 睡眠の規則 + hunger の下限 + health を減らさない(F5) | 睡眠の規則そのまま(計算上、週末に死ぬ)/ 減衰の上限 / 曜日スケジュール / 完全凍結 |
| A13 | 複数端末(Revision 2) | Standard 以降のオプション。端末ごとにファイルを分け、読み込み時にマージする | 1 ファイルを共有して上書き |
| A6 | 復活時の絆 | 新しいペットは絆 0 から。前のペットの MEMORIES はアルバムに永久保存 | 25% を引き継ぐ |
| A7 | コイン経済との接続 | **新しい稼ぎ口(ファウセット)は作らない**。MVP ではシグナル追加もしない。Standard で小さな見た目アイテムを使い道(シンク)として追加するかはレビュアーが判断 | 活動に応じてコインを付与 |
| A8 | 通知 | OS 通知は MVP・Standard では作らない | 進化時に通知の許可を求める |
| A9 | 打鍵検知 | 採用しない(Never) | Input Monitoring 権限で取得 |
| A10 | カレンダー | Ambitious 案のオプトインまで延期 | MVP に含める |
| A11 | 静寂時間の既定値 | 22:00–08:00 | 22:00–07:00 |

---

## Overview

Companion は、ペットが **Adult に到達した時点で有効になる**レイヤー。既存の世話ループ(`backend/src/pet/state.rs:100-185`)はそのまま残し、意味づけを変える。

- **世話を義務から挨拶へ**: 大人期は needs の減衰を 0.2 倍にし、世話は 1 日に FEED 1 回と PLAY 1 回で足りるようにする(F4)。
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
- アイドル時間の読み取り自体は、Companion より前に前提タスク P6(不在時間の扱い、Issue #16)で導入する。Companion はその値を共有する(IDLE_AWAY_MIN=10 分は表情のきっかけ、AWAY_THRESHOLD=30 分は needs を「深い眠り」に切り替える閾値で、別のノブ)。

**R5. 大人期の減衰**
- Adult の間、hunger・happiness・energy の減衰率に `ADULT_DECAY_MULT = 0.2` を掛ける(`needs.rs:4-6`)。health の増減ルールは変えない。
- Revision 2 で 0.4 → 0.2 に変更した(決定事項 18、承認済み 2026-10-14)。0.4 では、1 日 9 時間在席して毎日 FEED 1 回と PLAY 1 回をしても、9 営業日目に死ぬ(F4)。
- 前提: 端数の切り捨てバグ(前提タスク P2)が修正されていること。修正しないと、起動中はそもそも減衰しない(Formulas F0)。
- この倍率は在席時間の減衰と、不在時間の hunger 減衰(F5)の両方に掛ける。

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
- 死亡条件は変えない(`lifecycle.rs:3-19`)。演出は MVP から入れる。アルバム化は MEMORIES と同じく Standard から。
- **Revision 2**: 死は在席時間にだけ起きる。不在時間(F5)は「深い眠り」として扱い、health を減らさないので死なない。大人期に死ぬには、世話をしないまま在席時間が合計で約 46〜61 時間たまる必要がある(1 日 4〜9 時間の在席で 7〜12 営業日。F4)。在席が途切れず続いた場合の理論値は約 71 時間(F4)。
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
| Pet State / Needs(`state.rs`, `needs.rs`) | stage, needs, activity, is_alive | 大人期の減衰倍率 | Needs が倍率を受け取る(Needs 側が所有)。不在区間の判定と「深い眠り」の規則(F5、P6)も Needs 側が所有し、全ステージに適用する |
| Evolution(`evolution.rs`) | Adult 到達 | なし | Evolution |
| Lifecycle(`lifecycle.rs`) | 死亡、revive | Farewell 演出、Bond のリセット、アルバム化 | Lifecycle が通知、Companion が処理 |
| GameStats / Economy(`stats.rs`, `economy.rs`) | 世話・ゲームのイベント(コマンド成功時に Companion へ通知)。GameStats は累計しか持たない(`stats.rs:5-27`)ので、当日カウンタ n_care / n_focus と「その時点で need < 80 か」の判定は Companion が `companion.json` に持つ | **コインの増減なし**(MVP)。Standard では見た目アイテムのシンク案あり(決定事項 4) | Economy |
| Achievements(`achievements.rs`) | なし | 新しい実績候補 2 件。`Old Friends` は大人期に一緒にいた日数(手動オープンした日)が 30 日。`Memory Keeper` は MEMORIES 10 件(Standard)。Bond のティアは実績にしない(A3: 見せない絆が露出するため)。`check_achievements(stats, pet)`(`achievements.rs:87`)は Companion の状態を受け取れないので、引数に `&CompanionState` を追加するインターフェース変更が必要 | Achievements |
| Storage(`storage/mod.rs`) | 読み込み | `companion.json` の保存 | Storage |
| Renderer / UI(`renderer.js`, `main.js`) | なし | Mood ごとの目の描画、吹き出し、MEMORIES タブ | UI |
| Tray(`main.rs:30-61`) | クリック | アイコンと tooltip の差し替え | Tray |
| OS シグナル | ローカル時刻、アイドル秒数、電源状態 | なし | 共通の signals モジュール(Revision 2: P6-b で Companion より先に作り、Needs と Companion が共有する。アイドル秒数は P6-b、電源状態は Standard) |

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
  2. 経過時間は `Δt = clamp(now − last_update, 0, OFFLINE_CAP)` とする。OFFLINE_CAP は 96 時間(Revision 2 で 72 → 96。F5 の収束条件)で、needs に適用する(Revision 2: age には別の上限 AGE_OFFLINE_CAP を使う。F5)。時計が過去に戻った場合(負の値)は 0 にする。
  2b. **Revision 2**: Δt を「在席区間」と「不在区間」に分け、不在区間には F5 の「深い眠り」の規則を使う(前提タスク P6)。P2 と P6 は同じ逐次積分のループの中で扱う。
  3. **長い Δt は 1 分刻みで逐次積分する**(最大 96×60 = 5760 ステップ。起動時に 1 回だけ走る軽い処理)。各ステップで health の増減と死亡を判定する。睡眠が終わる時刻をまたぐ場合は、その時刻で区間を分ける。
     - 一括計算のままだと、health の判定が区間の終わりの状態で 1 回しか行われない(`needs.rs:41-47`)。そのため F4 の死亡時刻が約 33 時間に縮み、睡眠の途中終了も「最初から起きていた」扱いになる(`state.rs:81-89`)。
- 時計を巻き戻すと `(now − last_update) as u64` が巨大な値になる(`state.rs:75`)。この問題も同じ修正で塞ぐ。
- 補足: `needs.rs:34` の u8 加算は、睡眠が最長 4 時間で、減衰計算の前に起床処理が走る(`state.rs:81-86`)ため、現状ではオーバーフローに到達しない。逐次積分にすれば構造的にも起きない。

### F1. Bond の日次確定

**確定のタイミング**: その日最初の update(ローカル日付 ≠ `last_settle_date`)で 1 回だけ確定する。アプリが起動していなかった日があっても、ループは回さない。

`G = min(G_CAP, G_GREET·g + G_CARE·min(n_care, N_CARE_MAX) + G_FOCUS·min(n_focus, N_FOCUS_MAX))`(前回の確定以降に貯まった当日カウンタ。確定後に 0 に戻す)
`overdue(today) = min(max(0, m − GRACE), M_MAX)`(Revision 2: GRACE を 2 → 3 日にした。金曜に開いて月曜に開くと m=3 なので、週末だけでは Bond は減らない)
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
| GRACE | int | 3 | data file | 減衰しない猶予日数(Revision 2 で 2 → 3。週末をまたいでも減らない) |
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
  | MVP 標準・平日だけ(Revision 2) | 16(週 5 日) | 約 10 / 27 / 53 / 88 |
  | Standard 標準・平日だけ(Revision 2) | 約 24(週 5 日) | 約 7 / 18 / 35 / 59 |

  - 平日だけの行は、開いた日数に 7/5 を掛けた暦日数。GRACE=3 なので、週末に Bond は減らない。

  - 根拠: CLAUDE.md の「旅は意味のあるものに」(CLAUDE.md:22-28)に合わせ、Standard の標準的な使い方で最上位まで約 1.5 か月かかるようにした。
  - MVP では最上位まで約 2 か月かかるが、MVP のティアが解放するのは台詞プールだけなので許容する。
  - (Revision 2)オーナーのように平日だけ使う場合、Standard 標準でも最上位まで約 2 か月かかる。これも許容する。理由: 絆は見えない値で、最上位は長く一緒にいる人へのご褒美であり、途中のティアも 1〜5 週間で順に届く。短くしたい場合は G_CAP ではなく G_GREET を上げる(毎日開くことの価値が上がる)。
- **例**(GRACE=3): B=280 で、最後に開いた日(d0)の分はすでに確定済み。d1–d4 は開かず、アプリは起動したまま。
  - d3 の確定: m=3 なので overdue=0、D=0、B=280(週末明けの月曜に相当)。
  - d4 の確定: overdue=1、差分 1 なので D=8、B=272(下限 100 は割らない)。
  - d5 の確定: overdue=2、差分 1 なので D=8、B=264。アプリを閉じていて d5 に一度だけ確定した場合も、D=16 で同じ 264 になる。
  - d5 に挨拶・FEED 1 回(hunger < 80)・フォーカス 1 回をすると、d6 の確定で G=28、B=292 になる。
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
| ADULT_DECAY_MULT | float | 0.2(安全範囲 0.15–0.2。Revision 2 で 0.4 から変更) | data file | 大人期の倍率 |
| HEALTH_DECAY | float | 0.05 /分 | constant(`needs.rs:42`) | 危険状態での health 減少 |

- **出力**: 大人の減衰は hunger 0.04・happiness 0.03・energy 0.02 /分。
  - T_allzero = 100/0.02 = 5000 分 ≈ 83.3 時間(energy が最後に 0 になる)。
  - T_health = 90/0.04 = 2250 分(37.5 時間)+ 33.3 時間 ≈ **70.8 時間**。
  - よって、在席が途切れずに続けば **約 71 時間の完全放置で死亡**する(不在時間は数えない: 下の Revision 2 の項)。旧版の 0.4 では 41.7 時間だった。
  - 現行 Child は T_allzero = 100/0.1 = 1000 分 ≈ 16.7 時間。
- 前提: F0 の修正後(端数の蓄積 + 1 分刻みの逐次積分)の値。
- **Revision 2**: 上の T_death は、在席時間が途切れずに続いた場合の理論値。不在区間(F5)では health が減らず、energy が回復するので、全ステータス 0 による死も起きない。以下は現実の使い方で計算した値。
  - 条件: 月曜 9:00 に全ステータス 100 から始める。平日の在席時間以外はすべて不在とする。
  - 計算方法: `needs.rs` と `lifecycle.rs` の式を、P2 修正後の想定(f64・1 分刻み)で再現した。不在区間は F5 で計算した。
  - 在席時間の数え方は 2 通り示す。1 つは予定の在席時間だけを数える方法。もう 1 つは、不在の最初の 30 分(AWAY_THRESHOLD。F5 の定義で在席の規則を使う)も含める方法。

  **世話をしない場合の死亡時刻**

  | 倍率 | 在席 9 時間/日 | 在席 4 時間/日 |
  |---|---|---|
  | **Adult(0.2)** | 7 営業日目に死亡(在席 57.7 時間。境界を含めて 60.7 時間) | 12 営業日目(45.8 / 51.3 時間) |
  | 参考: 旧 Adult(0.4) | 5 営業日目(45.0 / 47.0 時間) | 10 営業日目(37.8 / 42.3 時間) |
  | 現行の全ステージ(1.0) | 5 営業日目(38.8 / 40.8 時間) | 9 営業日目(35.5 / 39.5 時間) |

  - 死因はどれも health が 0 になること。危険域(hunger < 10 または happiness < 10)にいた在席時間の合計が 33.3 時間に達すると死ぬ。happiness は不在中に回復しないので、hunger と並ぶ主な原因になる。夜間の不在で energy が 100 に戻るので、全ステータス 0 による死は起きない。

  **毎日の世話と生存**(在席 9 時間/日。FEED は 9:30、PLAY は 14:00 など、1 日の中で間隔を空ける。130 日まで計算)

  | 1 日の世話 | 0.4 | 0.3 | 0.25 | **0.2** |
  |---|---|---|---|---|
  | なし | 5 営業日目に死亡 | 6 | 6 | 7 |
  | FEED 1 | 7 | 8 | 9 | 11 |
  | **FEED 1 + PLAY 1** | 9 | 11(退勤直後の 30 分) | 17 | **生存** |
  | FEED 2 | 8 | 10 | 12 | 16 |
  | FEED 2 + PLAY 1 | 約 25〜26(世話の時刻で 1 日ずれる) | 生存 | 生存 | 生存 |
  | FEED 2 + PLAY 2 | 生存 | 生存 | 生存 | 生存 |

  - **目標**: 1 日 9 時間在席し、FEED 1 回と PLAY 1 回で無期限に生存する(「世話を義務から挨拶へ」)。これを満たすのは 0.2 以下だけなので、ADULT_DECAY_MULT を 0.2 にした(決定事項 18)。
  - 0.2 で世話をしないと、7〜12 営業日目に死ぬ。死の緊張は残る。
  - 0.2 で FEED 1 回 + PLAY 1 回の場合、1 日の収支は次のとおり(在席 9.5 時間 = 570 分)。
    - hunger: 消費 22.8、FEED +30、PLAY −5 で、差し引き +2.2。
    - happiness: 消費 17.1、FEED +5、PLAY +20 で、差し引き +7.9。
- 不在中の hunger は AWAY_HUNGER_FLOOR=30 で止まる。そのため翌朝の hunger は 30 前後から始まり、Adult(0.2)では FEED をしないと約 8 時間(20/0.04=500 分)で 10 を切る。「朝の FEED が挨拶になる」リズムを意図している。

### F5. 不在区間の判定と「深い眠り」(Revision 2、前提タスク P6 / Issue #16)

**区間の分割**(update のたびに、前回の `last_update` から今までの区間を在席と不在に分ける。在席が先、不在が後)

`Δ = clamp(now − last_update, 0, OFFLINE_CAP)`
- 空白による不在(アプリ非起動・Mac のスリープ・蓋閉じ): `Δ > AWAY_THRESHOLD` なら `present = AWAY_THRESHOLD`、`away = Δ − AWAY_THRESHOLD`。それ以外は `present = Δ`、`away = 0`。
- 離席による不在(アプリ起動中で Mac も起きているが、入力がない): `away = min(Δ, max(0, idle − AWAY_THRESHOLD))`、`present = Δ − away`。
- 両方が当てはまる場合は、away の大きい方を採る。

**深い眠りの規則**(不在区間の 1 分ごと。`needs.rs` の睡眠時の規則に、下限と health の保護を加えたもの)

`hunger ← max(hunger − HUNGER_DECAY_RATE × SLEEP_HUNGER_DECAY_MULT × stage_mult, min(hunger, AWAY_HUNGER_FLOOR))`
`happiness ← happiness`(睡眠中と同じく減らない: `needs.rs:25-29`)
`energy ← min(100, energy + SLEEP_ENERGY_RECOVERY_RATE)`(`needs.rs:32-34`)
`health`: 減少の分岐(`needs.rs:41-43`)は使わない。回復の分岐(`needs.rs:44-47`)はそのまま使う。

| Variable | Type | Range | Source | Description |
|---|---|---|---|---|
| idle | sec | 0–∞ | OS(`CGEventSourceSecondsSinceLastEventType`、権限不要) | 最後の入力からの秒数。読めない場合は 0(在席扱い)にする |
| AWAY_THRESHOLD | min | 30(安全範囲 10–60) | data file | この長さを超えた空白・離席を不在とみなす |
| AWAY_HUNGER_FLOOR | int | 30(安全範囲 20–50) | data file | 不在中に hunger が下がる下限。不在に入った時点ですでに下回っていれば、その値で止まる |
| HUNGER_DECAY_RATE / SLEEP_HUNGER_DECAY_MULT / SLEEP_ENERGY_RECOVERY_RATE | float | 0.2 / 0.5 / 0.5 | constant(`needs.rs:4,9,10`) | 既存の定数をそのまま使う |
| stage_mult | float | 1.0(Adult は 0.2) | R5 | ステージ別の減衰倍率 |
| OFFLINE_CAP | hour | 96 | constant | needs の積分ステップの上限(F0)。下の収束条件を満たす必要がある |
| AGE_OFFLINE_CAP | day | 14 | data file | 1 回の空白で age に加える時間の上限 |

- **死なないことの根拠**: 不在区間では health が減らない。energy は 1 分目から 0 より大きくなる。したがって `lifecycle.rs:9` と `lifecycle.rs:15` のどちらの死亡条件も、不在区間の中では成立しない。死亡判定は在席区間でだけ行えばよい。
- **収束**: 不在区間の needs は、hunger が下限に達した時点で変化しなくなる(health の回復も、hunger ≤ 50 か health = 100 で止まる)。全快からの収束時間は、Adult(0.2)が 30 分 + (98.8−30)/0.02 = 3470 分 ≈ 57.8 時間、倍率 1.0 が 670 分 ≈ 11.2 時間。
- **OFFLINE_CAP の制約**: `OFFLINE_CAP ≥ AWAY_THRESHOLD + (100 − AWAY_HUNGER_FLOOR) / (HUNGER_DECAY_RATE × SLEEP_HUNGER_DECAY_MULT × stage_mult_min)`。安全範囲の端(stage_mult 0.15、FLOOR 20)では 0.5 + 80/0.015/60 ≈ 89.4 時間になる。そのため OFFLINE_CAP は 96 時間にした。この制約を満たせば、上限で積分を打ち切っても結果は変わらない。
- **activity**: 不在区間で `activity` は書き換えない(戻ったときに FEED が「睡眠中」で拒否されないようにする)。もともと Sleeping だった場合も、`activity_until` を過ぎれば既存どおり Idle に戻る(`state.rs:81-86`)。
- **age**: 不在中も進める。`age += min(max(0, now − last_update), AGE_OFFLINE_CAP)`。理由は「§不在時間の扱い」の進化の項。
- **パッシブ収入**: 変更しない。既存どおり、空白 1 回につき最大 12 時間分(`stats.rs:33`)。

**計算例**(Adult、全ステータス 100 で金曜 18:00 に蓋を閉じる)
- 最初の 30 分は在席の規則: hunger 100 − 0.04×30 = 98.8、happiness 100 − 0.03×30 = 99.1、energy 100 − 0.02×30 = 99.4。
- 残りの 62.5 時間は深い眠り: hunger は 0.02/分で減り、約 57.3 時間後((98.8−30)/0.02 = 3440 分)に 30 で止まる。happiness は 99.1 のまま。energy は約 1.2 分で 100 になる。health は 100 のまま。
- 月曜 9:00 の値: 30 / 99.1 / 100 / 100(生存)。全シナリオの表は「§不在時間の扱い」。

---

## Edge Cases

- **既存セーブの pet がすでに Adult の場合**: 初回起動時に `companion.json` がなければ、B=0、`last_settle_date=今日`、`last_open_date=今日`、`overdue_applied=0` で作成する(猶予は通常の GRACE がそのまま効く)。次に開いたときに Onboarding を出す。遡ってのボーナスは付けない。
- **アプリを閉じている間に Adult に到達した場合**: 次の update で進化し、`companion_since=now` とする。最初に開いたときが Onboarding になる。
- **Companion 有効中に死亡した場合**: Farewell に遷移する。進行中のフォーカスは中止して報酬なし。Bond は確定済みの値のままアルバムに記録し、新しいペットは B=0 から始める。
- **フォーカス中にペットが死亡、またはスリープ/蓋閉じした場合**: 死亡なら即中止する。スリープ(壁時計と単調時計の差が SLEEP_DETECT_S=120 秒超)なら一時停止し、復帰後 FOCUS_RESUME_MIN=10 分以内に再開しなければ中止する(記録なし)。
- **時計を過去に戻した場合**: Δt は 0 として扱い、`last_update=now` にする。日付が `last_settle_date` より前なら、日次確定も当日カウンタのリセットもしない。
- **時計を未来へ大きく進めた場合**: needs の Δt は OFFLINE_CAP(96 時間)で止める。不在区間の needs は 96 時間以内に収束するので(F5 の制約)、本当に長く閉じていた場合と結果は変わらない。age は AGE_OFFLINE_CAP(14 日)で止める(Revision 2。旧版は age も 72 時間で止めていた)。時計を進めて得られるのは、1 回あたり最大 14 日分の成長だけになる。日次確定は 1 回だけで、overdue は最大 30。
- **タイムゾーン変更・DST の場合**: 日付境界はローカル日付で判定し、日付が変わったときに 1 回だけ確定する。現行のストリーク(`stats.rs:117`)は UTC 日付で判定しているため、Companion の日付と食い違いうる。ストリークも将来ローカル日付に揃えるべきで、Open Questions に記載した。
- **数日間アプリが起動していない場合**(Revision 2): 最初の AWAY_THRESHOLD 分だけ在席の規則で減衰し、残りは深い眠り(F5)になる。何日空いても死なない(週末・9 日・1 か月の数値は「§不在時間の扱い」)。Bond は F1 で m から一括計算し、日数分のループはしない。
- **アプリは起動中で Mac も起きているが、ユーザーがいない場合**(週末に職場の Mac をつけっぱなしにした等): アイドル時間が AWAY_THRESHOLD を超えた分は不在区間になる(F5)。アイドル時間が読めない環境では在席扱いになり、F4 の時間で死にうる。読めないことは起動時にログへ出す(黙って在席扱いにはしない)。
- **動画を見ている・長い文書を読んでいるなど、在席しているのに入力がない場合**: 不在とみなされ、dot は減衰しない。ユーザーに有利な誤判定なので許容する。
- **不在のあいだに dot が Sleeping だった場合**: 不在区間はすべて深い眠りの規則で計算する。`activity_until` を過ぎていれば、既存どおり Idle に戻す。
- **不在に入る時点で hunger が AWAY_HUNGER_FLOOR を下回っている場合**: hunger はその値のまま止まる(戻ってきたときに下がっていない)。happiness が 10 未満でも、不在中は health が減らない。戻ってからの在席時間では、既存どおり health が減る。
- **悪用: dot を終了してから Mac を使う**: アプリが動いていない時間は在席を判定できないので、不在として扱われ、減衰しない。防がない。影響は「生死の緊張がなくなる」ことだけで、Bond は手動で開かないと増えない(F1)。コインのパッシブ収入は既存の上限 12 時間のまま。
- **静寂時間が日をまたぐ場合**: F3 の `S > E` 側の式で判定する。S == E は静寂時間なしとする。
- **蓋閉じ・スリープ中のアイドル値**: スリープ中の値は使わない。復帰直後の WAKE_GRACE_S=60 秒はアクティブ扱いにし、`return_after_away` の誤発火を防ぐ。スリープ中の時間は、F5 の「空白による不在」で扱う(アイドル値がスリープ時間を含むかどうかは未確認なので、空白の判定とは独立させる)。
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
| OS シグナル | 上流 | Soft(HID アイドルは P6 経由で Hard) | ローカル時刻、HID アイドル、IOPowerSources | 未実装。HID アイドルは P6-b で先に入る |
| 不在時間の扱い(P6) | 上流 | Hard | F5 の区間分割と深い眠り | 未実装(Issue #16)。**Companion より先に入れる** |
| セーブ同期(P7) | 横断 | Soft | 端末別ファイルとマージ(§複数端末) | 未実装(Issue #17)。Standard 以降のオプション |

- 双方向の整合: 既存システムには GDD がない。各実装の PR で、逆方向の依存(例: Needs が大人期の倍率を受け取ること)をコードコメントかテストで明示する。

### 前提タスク(Companion の実装より先に行う)
| ID | 内容 | 根拠 | 重要度 |
|---|---|---|---|
| P6(Issue #16) | 不在時間を「深い眠り」として扱う(F5)。空白による不在を先に、離席による不在(HID アイドル)を P2 と同時に入れる。順序は「§不在時間の扱い」の実装順序 | 現行コードでは、典型的な状態で一晩(約 8 時間)蓋を閉じただけでも死にうる(§不在時間の扱い)。オーナーは週末に世話ができない | **最高** |
| P1(Issue #11) | セーブを原子的に書く(tmp → fsync → rename)。`.bak` を残す。パースに失敗したら退避し、黙って新しいペットにしない | `lib.rs:16-26` が失敗時に `PetState::new()` を返し、次の保存で上書きして永久に失われる。`storage/mod.rs:29` の `fs::write` は非原子的 | 高 |
| P2(Issue #12) | needs の端数を蓄積する。Δt を [0, OFFLINE_CAP] に clamp する。長い Δt は 1 分刻みで逐次積分する | F0 | 高 |
| P3 | ポップオーバーが非表示の間は rAF ループと 5 秒ポーリングを止め、Rust 側の 60 秒 tick に集約する。パッシブ収入の `accrue_passive` もポーリング経由で呼ばれている(`commands.rs:61`)ので、tick 側へ移す(そうしないと非表示中にコインが貯まらなくなる) | `renderer.js:61` に停止条件がない。`main.js:70` が常時動いている | 中 |
| P4(Issue #13) | ゲーム報酬をサーバー側で検証する(開始トークン、最短プレイ時間)。日次上限 30 枚 | `commands.rs:191-203` がクライアントの申告をそのまま信じている。devtools から 1 回最大 10 枚(memory: `economy.rs:39-43`)を無限に稼げる | 中(経済) |
| P5 | README の保存先の記述を修正する(実際は `~/Library/Application Support/dot/`) | `dirs::config_dir()`(`storage/mod.rs:7`)と README.md:79 が食い違う | 低 |
| P7(Issue #17) | 複数端末のセーブ同期(§複数端末とセーブ同期)。**P1 が前提** | 職場と自宅の 2 台で同じ dot を使いたい。週末問題の解決には使わない(P6 で解決する) | 低(Standard 以降のオプション) |

## Tuning Knobs

| Knob | 初期値 | 安全範囲 | 上げすぎると | 下げすぎると | 根拠 |
|---|---|---|---|---|---|
| ADULT_DECAY_MULT | 0.2 | 0.15–0.2 | 0.2 を超えると、在席 9 時間/日では FEED 1 + PLAY 1 で維持できない(F4 の表) | 世話の意味が消え、死もほぼ起きない(0.15 で世話なしなら、9 時間/日の在席で 8 営業日目に死亡。在席 65.3 時間) | Revision 2 で 0.4 → 0.2(決定事項 18)。在席が途切れなければ約 71 時間で死ぬ。不在時間は数えない(F5) |
| G_GREET | 10 | 5–15 | 開くだけで育つ | 毎日開く動機が弱まる | 日次上限の 1/5 |
| G_CARE / N_CARE_MAX | 3 / 5 | 2–5 / 3–8 | 世話の連打が有利になる | 世話と絆のつながりが見えない | 最大 15 で上限の 30% |
| G_FOCUS / N_FOCUS_MAX | 15 / 2 | 10–20 / 1–3 | 生産性アプリ化する(アンチピラー 2) | フォーカスの存在意義が薄れる | 加点の主役にしない |
| G_CAP | 50 | 30–80 | ティアがすぐ尽きる | 最上位まで 2 か月を超える | 毎日開く場合、Standard で最上位まで最速 20 日、標準 42 日。平日だけなら標準で約 59 日(F1 の表) |
| GRACE / DECAY | 3 日 / 8 | 3–5 / 0–15 | (GRACE を上げると)放置が無害化する | (GRACE が 3 未満だと)週末のたびに Bond が減る。(DECAY を上げると)罪悪感を生む | ティア下限で守るので穏やかに。Revision 2 で GRACE を 2 → 3(金曜→月曜は m=3) |
| Bond ティア | 100/300/600/1000 | — | — | — | Standard 標準(毎日開く)で 5/13/25/42 日。平日だけなら 7/18/35/59 日 |
| C / R / GAP / D_MAX | 3 / 4h / 45 分 / 5 | 2–4 / 2–6h / 30–90 分 / 3–6 | 邪魔になる | 存在が感じられない | F3 |
| 静寂時間 S–E | 22–08 | ユーザー設定 | — | — | ux-designer 案 |
| MOOD_MIN_DWELL | 600 秒 | 300–1800 | 反応が鈍い | 表情がちらつく | systems 案 |
| AWAY_RETURN_H / MISSED_YOU_H | 4 時間 / 24 時間 | 2–8 / 12–48 | 挨拶が稀になる | 毎回 `WELCOME BACK` になる | 業務日の昼休みを越える程度 |
| IDLE_AWAY_MIN(Standard) | 10 分 | 5–30 | 復帰の検知が遅れる | 席を立つたびに反応する | ux 案 |
| WAKE_GRACE_S | 60 秒 | 30–180 | 復帰直後の離席判定が遅れる | スリープ復帰のたびに `return_after_away` が誤って出る | Edge Cases「蓋閉じ・スリープ中のアイドル値」 |
| STREAK_WINDOW_D(P7) | 60 日 | 30–120 | 端末ファイルが大きくなる | 長いストリークを端末をまたいで計算し直せない(longest は最大値で保つ) | §複数端末のマージ規則 |
| SIGNAL_TICK(Standard) | 60 秒 | 30–120 | 反応が遅れる | CPU を使う | TD 案。非表示中の CPU を 0.1% 未満にする |
| TRAY_SWAP_MIN | 60 秒 | 30–300 | — | ちらつく | TD 案 |
| FOCUS_LEN | 25 分 | 15–50 | — | — | ポモドーロ慣行 |
| LINE_MAX_CHARS | 24 | 16–30 | 吹き出しに収まらない | 意味が伝わらない | 280px 幅、pixel 書体 |
| BUBBLE_TTL | 6 秒 | 4–10 | 画面に残りすぎる | 読み切れない | ux 案 |
| LINE_SPEAK_P | 0.5 | 0.2–0.8 | おしゃべりになる | 存在感が消える | 「沈黙も表現」(Game Feel) |
| LINE_HISTORY / LINE_CATEGORY_COOLDOWN | 20 件 / 24 時間 | 10–40 / 12–48 | (下げると)同じ台詞が目立つ | — | ux 案 |
| OFFLINE_CAP | 96 時間 | F5 の制約式の値(既定のノブで約 58 時間、安全範囲の端で約 89 時間)〜168 | 起動時の積分ステップが増える(96 時間で 5760 回) | 不在区間の収束時間を下回ると、長い不在の結果が変わる | Revision 2: 不在では死なないので、「T_death より長くする」という Review 1 #1 の制約はなくなった。役割はステップ数の上限と、時計を進めたときの影響の制限 |
| AWAY_THRESHOLD | 30 分 | 10–60 | 短い不在(会議・昼食)でも減衰する時間が増える | 短い離席や読書でも眠ったことになり、在席中の減衰が減る | F5。会議 1 本より短く、休憩より長い値 |
| AWAY_HUNGER_FLOOR | 30 | 20–50 | 戻ったときに世話の必要がない(FEED の挨拶が消える) | 20 未満では、戻った瞬間に `concerned` になる(CONCERNED_ENTER=20)。10 未満では在席に戻った直後から health が減る | F5。`hungry`(< 20: `state.rs:208`)より上 |
| AGE_OFFLINE_CAP | 14 日 | 3–30 | 時計を進めて大きく成長できる | 長い休暇のあとに成長が止まって見える | F5。ゴールデンウィーク・年末年始(最大 10 日程度)を取りこぼさない |
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
- Issue(Keitinn/dot): #16(不在時間 → P6・F5・§不在時間の扱い)、#17(複数端末 → P7・§複数端末とセーブ同期)、#10(進化期間 → §不在時間の扱いの age の項)、#11(P1)、#12(P2)、#13(P4)

---

## 導入計画: 3 案と推奨

| 案 | 範囲 | 権限 | 経済 | 工数感 | 評価 |
|---|---|---|---|---|---|
| **最小(MVP)** | 前提タスク P6・P1–P3、R1(オンボーディング)・R2(Mood)・R3(台詞、ポップオーバー内のみ)・R4(開いた時刻による不在判定)・R5(大人期の減衰)・R6(Bond の内部蓄積、ティアは台詞プールのみ解放)・R10(Farewell 演出。アルバムなし)、設定(マスター/静寂時間/QUIET) | なし(時刻のみ) | 接続なし | 小 | Adult に意味が生まれる。邪魔になるリスクはゼロ |
| **標準(推奨の到達点)** | MVP + R4 の離席検知(HID アイドル、権限不要)・R7 MEMORIES・R8 フォーカス・R9 トレイ表情・バッテリー低下の一言(非充電で 20% 未満、ポップオーバー/トレイのみ)、P4(ゲーム報酬の検証) | なし(TCC のプロンプトは 0 回) | 新しい稼ぎ口なし。見た目アイテムのシンクは決定事項 4 次第 | 中 | 閉じている間も「そばにいる」感じが出る |
| **野心的** | 標準 + オプトインのネイティブ通知(1 日 3 回まで)、オプトインのカレンダー(EventKit、予定名は表示しない)、カメラ/マイク使用中の静音、季節の MEMORIES | 通知の許可、カレンダー(TCC) | 季節アイテム(ライブ運用) | 大 | 未署名の配布ではリリースごとに TCC の許可が外れる(TD)。P1 のピラーと緊張関係にある |

**推奨**: 標準を到達点とし、**MVP → 標準の 2 段階で出す**。最初のリリースは MVP(前提タスク込み)で、そこで「開いたときの一言」が鬱陶しくないかを確かめてから、トレイ表情を足す。野心的な案は、署名と公証付きの配布に移るまで凍結する。

### プライバシーと macOS の権限
| シグナル | API | TCC | 採否 |
|---|---|---|---|
| ローカル時刻 | `chrono::Local` | 不要 | MVP |
| アイドル秒数 | `CGEventSourceSecondsSinceLastEventType` / IOKit `HIDIdleTime` | 不要 | **MVP の前提(P6-b)**。Companion の離席検知(標準)も同じ値を使う |
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

## 不在時間の扱い(前提タスク P6 / Issue #16)【最優先】

### 問題
- オーナーは職場の Mac で dot を使う。週末は世話ができない。金曜 18:00 → 月曜 9:00 は 63 時間で、旧版 F4 の死亡ライン(Adult 約 42 時間。倍率 0.4)を超える。このままでは週末ごとに「別れ」が起きる。
- **現行コードはもっと厳しい**。`state.rs:75` の経過時間を `needs.rs:22,27,36` が一度に掛けるので、閉じていた時間がそのまま減衰になる(`f64 as u8` は 255 で飽和する)。計算上、全ステータス 100 からでも 16.7 時間(1000 分。energy が最後に 0 になる)閉じていれば、全ステータスが 0 になって死ぬ(`lifecycle.rs:15`)。金曜夕方の典型値(hunger 70 / happiness 70 / energy 50)なら、8.3 時間(500 分)で死ぬ。つまり**一晩蓋を閉じただけで死にうる**。実機での再現は未確認。
- iCloud 等の同期では解決しない。週末に両方の Mac を触らなければ、同じように死ぬ。
- **決定: 放置の扱いを先に直す**(オーナー回答)。

### 方針: 不在時間は「dot が眠っている」(推奨)
- 不在区間(F5)では、dot は深く眠っている。needs は睡眠時の規則(`needs.rs:9-10, 21-38`)で動き、さらに次の 2 つを加える。
  1. hunger は AWAY_HUNGER_FLOOR=30 より下がらない。
  2. health は減らない(回復はする)。
- **2 つを加える理由(数値で確認済み)**: 睡眠の規則だけでは、hunger が半分の速さで減り続ける。hunger が 10 を切ると health が減り(`needs.rs:41-43`)、死ぬ。
  - 倍率 1.0(現行の全ステージ。Companion 前の Egg〜Child も同じ)は、全ステータス 100 からでも 47.8 時間、金曜夕方の典型値からなら 42.8 時間で死ぬ。**週末(63 時間)を越えられない**。
  - Adult(0.2)は、全快から 107.8 時間、典型値から 82.8 時間で死ぬ。週末は越えられるが、**9 日の連休は越えられない**。
  - happiness が 10 未満で不在に入ると、どのステージでも 33.3 時間で死ぬ(睡眠中は happiness が回復しないので、health が減り続ける)。
- 「眠っているから世話の義務感が生じない」という「待っていてくれる」(Player Fantasy)の核と整合する。戻ると dot は休んで元気(energy 100)で、少しお腹を空かせている(hunger 30)。最初の FEED が「おかえり」の挨拶になる。
- 世話の意味は「生死を守る」から「関係を育てる」(Bond・Mood・MEMORIES)へ移す。生死の緊張は在席時間にだけ残る(F4)。

### 代替案との比較

| 案 | 中身 | 悪用(わざと閉じて放置) | 世話の意味の希薄化 | 実装コスト | 既存セーブへの影響 | 週末 63 時間の結果 |
|---|---|---|---|---|---|---|
| **推奨: 深い眠り** | 睡眠の規則 + hunger 下限 30 + health を減らさない | アプリを終了して Mac を使えば、減衰を避けられる。防げないが、Bond は増えないので得るものは小さい | 中。生死の緊張は在席時間だけに残る。代わりに朝の FEED と Bond に意味を置く | 中(区間分割 + 1 分刻みの積分 + HID アイドル) | なし(既存の `last_update` だけを使う。新しいフィールドは不要) | 全シナリオで生存 |
| 睡眠の規則そのまま | `needs.rs` の睡眠時の規則だけを適用 | 同上 | 小 | 小 | なし | 倍率 1.0 は**死ぬ**(典型値から 42.8 時間)。Adult(0.2)は週末を越えるが、9 日では死ぬ。happiness < 10 なら 33.3 時間で死ぬ |
| (a) 減衰に上限 | 起きているときの規則で減衰し、OFFLINE_CAP を死亡ライン未満にする | 同上 | 小。ただし毎週月曜に瀕死の dot と会うことになり、罪悪感を生む(アンチピラー 3) | 小 | なし | 上限 36 時間: 倍率 1.0 は全快でも 16.7 時間、Adult(0.2)も低調・happiness 危険の状態からは死ぬ。上限 12 時間: 倍率 1.0 の典型値は 8.3 時間で死ぬ。上限 8 時間: 倍率 1.0 の低調な状態は 3.3 時間で死ぬ。生き残った場合も月曜の hunger は 0〜81 とばらつく。**出発時の状態で死亡時刻が変わるので、固定の上限では守れない** |
| (b) 曜日スケジュールの休止 | 設定した曜日(例: 土日)は減衰しない | すべての曜日を休止にできる | 小 | 中(設定 UI と曜日の判定。タイムゾーンも扱う) | なし(既定は空) | 設定していれば生存。**平日の夜・病欠・祝日・連休は守れない**ので、単独では足りない |
| (c) 完全凍結 | 不在のあいだ、needs も health も一切変えない | 推奨案と同じ | 推奨案と同じ | 小(推奨案より簡単) | なし | 全シナリオで生存。戻ったときの値は出発時と同じ |

- (c) は推奨案の次に良い。推奨案を選ぶ理由は、オーナーの「眠っている扱い」という言葉と物語が合い、戻ったときに「休んで元気・少し空腹」という変化が見えるため。実装を最小にしたいなら (c) に切り替えてよい(推奨案から下限と energy の回復を外したものに近い)。

### 数値の検証(推奨案)
条件: `needs.rs` と `lifecycle.rs` の式を、P2 修正後の想定(f64・1 分刻み)で再現した。不在の最初の 30 分(AWAY_THRESHOLD)は在席の規則、残りを深い眠りで計算した。値は hunger / happiness / energy / health。「倍率 1.0」は、Companion を入れる前の全ステージ(Egg〜Adult)にあたる。

| 出発時の状態 | 倍率 | 6 時間後 | 12 時間後 | 24 時間後 | **63 時間(週末)** | **9 日(GW)** | **30 日** |
|---|---|---|---|---|---|---|---|
| 全快 100/100/100/100 | Adult 0.2 | 92.2/99.1/100/100 | 85.0/99.1/100/100 | 70.6/99.1/100/100 | 30/99.1/100/100 | 30/99.1/100/100 | 30/99.1/100/100 |
| 全快 | 1.0 | 61.0/95.5/100/100 | 30/95.5/100/100 | 30/95.5/100/100 | 30/95.5/100/100 | 30/95.5/100/100 | 30/95.5/100/100 |
| 金曜夕方の典型 70/70/50/100 | Adult 0.2 | 62.2/69.1/100/100 | 55.0/69.1/100/100 | 40.6/69.1/100/100 | 30/69.1/100/100 | 30/69.1/100/100 | 30/69.1/100/100 |
| 典型 | 1.0 | 31.0/65.5/100/100 | 30/65.5/100/100 | 30/65.5/100/100 | 30/65.5/100/100 | 30/65.5/100/100 | 30/65.5/100/100 |
| 低調 20/20/20/50 | Adult 0.2 | 18.8/19.1/100/50 | 同左 | 同左 | 同左 | 同左 | 同左 |
| 低調 | 1.0 | 14.0/15.5/100/50 | 同左 | 同左 | 同左 | 同左 | 同左 |
| happiness だけ危険 60/5/60/100 | Adult 0.2 | 52.2/4.1/100/98.5 | 45.0/4.1/100/98.5 | 30.6/4.1/100/98.5 | 30/4.1/100/98.5 | 30/4.1/100/98.5 | 30/4.1/100/98.5 |
| happiness だけ危険 | 1.0 | 30/0.5/100/98.5 | 同左 | 同左 | 同左 | 同左 | 同左 |

- **すべてのシナリオで生存する**。Adult(0.2)は約 58 時間、倍率 1.0 は約 11 時間で値が変わらなくなる(F5 の収束)。そのため 9 日と 30 日の結果は、63 時間と同じになる。
- 低調な行の hunger が 20 ではないのは、最初の 30 分(在席の規則)で減ったあとの値で止まるため(下限の式 `min(hunger, 30)`)。
- health が 100 から下がるのは、最初の 30 分(在席の規則)で危険域にいた場合だけ(最後の行: −1.5)。
- **死ぬ条件**: 不在区間の中では死なない(F5 の根拠)。死ぬのは次の 2 つの場合だけ。
  1. 不在に入る前から瀕死で、最初の AWAY_THRESHOLD 分(在席の規則)の間に health が 0 になる場合。たとえば happiness < 10 で health ≤ 1.5 の状態で蓋を閉じた場合。
  2. 戻ってからの在席時間に世話をしない場合(F4 の表)。
- 比較(同じ条件で計算): 現行コードは全シナリオで死ぬ。旧版の設計(P2 修正 + 起きているときの規則 + 大人期 0.4 倍)は、Adult が全快から 41.7 時間、典型値から 20.9 時間で死ぬ。

### 「死は在席中の放置でのみ起きる」という原則
- **採用する**。定義: 在席時間 = アプリが起動中で、Mac が起きていて、最後の入力から AWAY_THRESHOLD 以内の時間。
- オーナー案の「起動中の完全放置でのみ」より範囲を狭くした。理由: 職場の Mac は週末もつけっぱなしのことがある。「起動中」だけを条件にすると、離席による不在(HID アイドル)を入れない限り、週末に死ぬ。
- 決定事項 7 をこの原則に合わせて修正した(末尾)。

### OFFLINE_CAP と進化(age)
- OFFLINE_CAP は残し、72 時間から 96 時間に延ばす。役割は、起動時の積分ステップの上限(5760 回)と、時計を進めたときの影響の制限。F5 の制約式を満たすので、上限が結果を変えることはない(Adult 0.2 の収束は約 58 時間で、72 時間でも足りる。ただしノブを安全範囲の端にすると約 89 時間になる)。Review 1 #1 の「T_death より長くする」という制約は、不在では死ななくなったので不要になった。
- **age は不在中も進める**(推奨)。上限は空白 1 回につき AGE_OFFLINE_CAP=14 日。
  - 理由 1: 眠っている間も育つ、という物語に合う。
  - 理由 2: 進化期間を日単位にする Issue #10(保留中)では、カレンダーの日数で旅を感じさせたい。平日しか起動しない人で age が止まると、Child → Adult(1 週間以上の案)に 2 週間近くかかってしまう。
  - 理由 3: 現在の閾値(合計 6 時間: `evolution.rs:24-26`)では、この判断はほとんど影響しない。影響が出るのは #10 を入れてから。#10 を再開するときに AGE_OFFLINE_CAP を見直す。
  - 代替: age も在席時間だけで進める。「一緒にいた時間で育つ」(P3)には合うが、職場だけで使う場合は Adult まで 1 か月以上かかる(#10 の案では)。
- 旧版の Edge Cases にあった「長期不在のあとの成長は 72 時間分で止まる」は、AGE_OFFLINE_CAP の導入で 14 日に変わる。

### F0(P2)との整合
- P2 の修正(端数の蓄積、Δt の clamp、1 分刻みの逐次積分)は、F5 の区間分割とループを共有する。1 分刻みのループの中で、そのステップが在席か不在かを見て規則を選ぶ。
- P2 だけを先に入れると、起動中の減衰が実際に効き始める(現在は切り捨てでほぼ 0: F0)。そのため、**P2 は離席による不在(HID アイドル)と同時に出す**。そうしないと、つけっぱなしの職場 Mac で週末に死ぬ。

### 実装順序(別 PR・別 Issue で行う。この節は仕様)
1. **P6-a 空白による不在**(Issue #16): `update` で Δt を clamp し(負の値は 0)、F5 の空白の規則で present / away に分ける。away には深い眠りの規則を当てる。現行の整数の減衰のままでも入れられ、週末と夜間の死を止める。単体テスト: 63 時間・9 日・30 日の表の値(±1)と、時計を戻した場合。
2. **P6-b + P2 離席による不在と端数の修正**(Issue #16 と #12): Rust 側に 60 秒の tick を置き、`CGEventSourceSecondsSinceLastEventType` を読む(P3 の tick と共用)。`update` にアイドル秒数を渡せるようにし(テストでは差し替える)、1 分刻みの積分に移す。単体テスト: F4 の在席時間の表。
3. **age の上限**: age に AGE_OFFLINE_CAP を適用する(P6-a に含めてよい)。
4. **P1**(Issue #11)は並行して進めてよい(独立している)。
5. **Companion MVP**: R5 の大人期の倍率を、在席と不在の両方に掛ける。

---

## 複数端末とセーブ同期(前提タスク P7 / Issue #17、Standard 以降のオプション)

週末問題は P6 で解決する。同期は「職場と自宅で同じ dot に会いたい」ための別の機能として扱う。

### 現状の問題(コード根拠)
- **アトミックでない全体上書き**: `save_state` / `save_stats` は `fs::write` でファイル全体を上書きする(`storage/mod.rs:26-31, 45-50`)。書き込み中にクラッシュしたり、同期クライアントが途中のファイルを読んだりすると、壊れた JSON になりうる。
- **読み込みは起動時の 1 回だけ**: `AppState::new` が起動時に読み(`lib.rs:14-32`)、以降はメモリ上の値を 5 秒ごとに書き戻す(`main.js:70-73` → `commands.rs:53-65` で `save_pet` と `save_stats`)。もう一方の端末の変更は、再起動するまで取り込まれず、次の保存で上書きして消える。
- **破損時に黙って新規ペットにする**(P1): パースに失敗すると `PetState::new()` / `GameStats::new()` を返し(`lib.rs:16-26`)、次の保存で上書きする。同期中の不完全なファイルを 1 回読んだだけで、ペットもコインも失われる。
- したがって **P1(Issue #11)の修正が前提**になる。

### accrue_passive の二重加算(コードを読んで確認)
- 仕組み: `GameStats` は `last_passive_time` を 1 つだけ持つ(`stats.rs:25-26`)。`accrue_passive` は前回の支払いからの経過を 10 分(`PASSIVE_INTERVAL_SECS`)ごとに 1 枚払い、空白 1 回につき最大 12 時間分(`PASSIVE_MAX_SECS`)まで払う(`stats.rs:29-95`)。呼ばれるのは `get_pet_state` の中(`commands.rs:61`)で、5 秒ごとのポーリングから来る。
- **現行コードのまま同期フォルダに置いた場合**: 二重加算ではなく、**上書きによる消失**が起きる。2 台とも起動していると、それぞれが自分のメモリ上の `last_passive_time` で同じ時間帯に支払う(メモリ上では 10 分あたり合計 2 枚)。しかし保存は全体の上書きなので、後に書いた端末の値だけが残る。もう一方で増えたコイン・実績・カウンタは消える。
- **加算的なマージを素朴に入れた場合**: **二重加算が起きる**。2 台が同じ時間帯にそれぞれ支払い、その差分を足し合わせるため。最大で、両方が起動している時間の 2 倍になる。また、古いファイルを読んだ端末が、もう一方がすでに払った時間帯に対して、空白の上限 12 時間分(72 枚)を払いうる。
- 毎日の世話のボーナス(`update_streak` の `add_coins`: `stats.rs:145-146`)も同じ構造で、2 台で同じ日に 1 回ずつ払われうる。
- **対策(推奨)**: パッシブ収入を「時間帯の番号」で記録する。時間帯の番号 k = floor(t / 600) とする。各端末は、自分が払った k の範囲を台帳(device id 別)に記録する。合計のパッシブ収入は、全端末の範囲の**和集合の大きさ**とする。同じ時間帯を 2 台が払っても 1 回と数える。毎日のボーナスも同様に「払った日付の和集合」で数える。
  - 現行の `accrue_passive` は、前回の支払い時刻から 600 秒ずつ進める方式で(`stats.rs:91`)、時間帯の区切りに揃っていない。移行時は、最初の支払いの起点を `ceil(last_passive_time / 600)` の区切りに切り上げる(端数の最大 10 分ぶん、1 枚未満を切り捨てる)。以後は区切りごとに払う。
  - 生きているかどうかは、時間帯ごとではなく、支払いを行う時点の判定のままにする(`stats.rs:77-80` と同じ)。

### 保存の形: 端末ごとのファイル(推奨)
- 同期フォルダに `dot/devices/<device_id>.json` を置く。各端末は**自分のファイルにだけ**書く。読むときは全端末のファイルをマージする。
- 同じファイルに 2 台が書かないので、iCloud の競合コピー(`pet-state 2.json` 等)は通常は生まれない。
- device id は初回起動時に作る UUID で、同期しないローカルの `~/Library/Application Support/dot/device.json` に置く。移行アシスタントや復元で `device.json` が別の Mac にコピーされると、同じ id の端末が 2 台になる。これを防ぐため、ハードウェア UUID(`IOPlatformUUID`、権限不要)のハッシュも一緒に記録し、一致しなければ id を作り直す。
- ローカル(Application Support)の保存は今までどおり残す。同期フォルダは複製であり、読めなくてもローカルの状態で動き続ける。

### マージ規則
マージは可換・冪等にする(同じファイルを 2 回読んでも、順番が変わっても、結果は同じ)。そうすれば、遅れて届いたファイルや競合コピーも同じ規則で取り込める。

| データ | 規則 | 理由 |
|---|---|---|
| PetState(needs・activity・age 等) | **最後に世話をした時刻(`last_action_at`、新設)が新しいスナップショットを採る**。同じなら、前回の世話以降の在席時間(`present_secs_since_action`、新設。世話のたびに 0 に戻し、在席区間の秒数を足す)が長い方。さらに同じなら device id の小さい方 | オーナー案の「`last_update` が新しい方」はそのままでは使えない。起動中の端末は 5 秒ごとに `last_update` を更新するので(`state.rs:97`)、何もしていない端末が常に勝ってしまう |
| ペットの同一性 | スナップショットは `PetState.id` ごとに扱う。死亡も「その時刻のスナップショット」として比べる。死亡より後に同じ id で世話をした記録があれば、生きている方を採る | 片方で死亡・復活した後に、もう片方で古いペットの世話をしていた、という競合に決定的な答えを出す |
| age | 採ったスナップショットの値と、全端末の最大値のうち大きい方 | 成長は戻らない |
| GameStats のカウンタ(total_feeds 等) | 端末ごとの累計を持ち、端末ごとに最大値を採ってから合計する(G-Counter) | 上書きで消えない。同じファイルを 2 回読んでも増えない |
| コイン | 「端末ごとの獲得(パッシブ以外)の合計」+「パッシブの時間帯の和集合」−「端末ごとの支出の合計」 | 二重加算と消失の両方を防ぐ |
| 実績 `unlocked` | 和集合 | 一度解放したものは失わない |
| ストリーク | 世話をした日付の集合(直近 STREAK_WINDOW_D=60 日)の和集合から計算し直す。longest は最大値 | 端末をまたいだ連続日も数える |
| companion.json の Bond | 日付ごとの当日カウンタを端末ごとに記録する。マージでは日付ごとに、g は論理和(どちらかで開いていれば 1)、n_care と n_focus は合計にする。そのうえで F1 を日付順に当て直す。B はこの結果から決まる。最後に手動で開いた日(m の計算に使う)は最大値を採る。日別カウンタは今のペットが生まれてからの全期間を残す(1 日 1 行で、1 年でも数十 KB)。死亡時にアルバムへ移す | 端末ごとに確定すると、日次上限 G_CAP が 2 倍になる。g を合計にすると、2 台で開いた日に挨拶が 2 回分数えられる |
| MEMORIES | 和集合(解放日は早い方) | 失わない |
| Companion の設定(静寂時間・TRAY FACE 等) | **同期しない**(端末ごと) | 職場と自宅で静寂時間が違うのが自然 |
| オンボーディングの回答 | どちらかで YES なら YES | 同じ質問を 2 回しない |

### 同期を初めて有効にするとき(2 台に別々のペットがいる場合)
- オーナーは両方の Mac にすでに dot を入れている可能性が高い。その場合、同期フォルダには別の id のペットが 2 匹いることになる。
- ペットは 1 匹しか選べない。初回のマージで別の id の生きたペットを見つけたら、ポップオーバーに `WHICH DOT?` を出し、両方の stage と年齢を並べてユーザーに選んでもらう(モーダルは使わない)。選ばれなかったペットはアルバムに移す(Standard。MVP ではアーカイブファイルに退避する)。Bond の日別カウンタはペット id ごとに記録し、選ばれたペットの分だけで F1 を当て直す(選ばれなかったペットの分は、そのペットと一緒にアルバムへ移す)。選ぶまでは、各端末が自分のペットのまま動く。
- GameStats(カウンタ・コイン・実績・ストリーク)は、どちらのペットを選んでも端末別の台帳として合算する。2 台で別々に貯めたコインは足し合わされる。これは二重加算ではなく、実際に 2 台で稼いだ分なので許容する。パッシブ収入の時間帯は、和集合で重複を除く。

### 2 台とも起動している場合
- 同期フォルダの他端末のファイルは、ポップオーバーを開いたときと、60 秒の tick ごとに更新時刻を見て、変わっていれば読み直す。
- 同時に世話をした場合(職場で FEED、同じ時間帯に自宅でも FEED): PetState は後から世話をした方を採るので、先の FEED の効果(hunger +30)は消える。カウンタ・Bond の当日カウンタ・コインは端末ごとに記録しているので消えない。2 台の前に同時に座ることはまれなので、これを許容する。
- 片方が不在(アイドル)で、もう片方で在席している場合: 在席している方のスナップショットは減衰が進んでおり、世話の時刻が同じなら「在席時間が長い方」の規則でこちらが採られる。dot は 1 匹で、ユーザーがどちらかの前にいれば在席とみなす。
- 支出の競合: 2 台がオフラインで同時に同じコインを使うと、合計が負になりうる。負の残高は 0 と表示し、正に戻るまで新しい購入を止める(借金として扱う)。めったに起きないので、これ以上の対策はしない。

### 書き込み・遅延・退避への備え
- **アトミック書き込み**: 同じディレクトリに一時ファイルを書き、fsync してから rename する(P1)。一時ファイルを同期クライアントがアップロードするかどうか(先頭が `.` の名前で避けられるか)は未確認。
- **競合コピー**: `<name> 2.json`(iCloud)や `<name> (… conflicted copy …).json`(Dropbox)を見つけたら、同じマージ規則で取り込んでから `conflicts/` に退避する。マージが冪等なので、取り込んでも値は増えない。
- **同期の遅延**(数秒〜数分。iCloud はそれ以上のこともある): マージが可換なので、遅れて届いても結果は同じ。届くまでの間は、他端末の最新の世話が反映されない。
- **退避(iCloud の evict)**: 「Mac のストレージを最適化」が有効だと、ファイルが実体のないプレースホルダになりうる。普通のファイル読み込みで自動的にダウンロードされるかどうかは未確認。読めない・タイムアウトした端末のファイルはその回は飛ばし、前回のマージ結果を使う。**読めないことを「新しいペット」と解釈してはいけない**(P1 と同じ方針)。
- **時計のずれ**: 端末間の時刻は NTP でほぼ揃っている前提。数秒のずれは、世話の時刻の比較で同点に近い場合にしか影響しない。

### 段階導入
1. **前提**: P1(Issue #11)。原子的な書き込みと、読み込みの失敗を新規ペットにしないこと。
2. **段階 0: 端末別ファイルとマージをローカルで作る**。2 つのフォルダで 2 台を模擬し、単体テストでマージ規則を確かめる。同期サービスにはまだ触らない。
3. **段階 1: ユーザーが指定するフォルダにペットのファイルを置く**(オーナー決定 2026-10-14)。設定に `SYNC FOLDER` を追加し、ユーザーがフォルダを指定する。そのフォルダの中の端末別ファイル(`devices/<device_id>.json`)に書き、読むときにマージする。どの同期サービスを使うか(iCloud Drive、Dropbox、外付けディスクなど)は dot の仕様の外で、dot はフォルダの中身を読み書きするだけにする。フォルダ選択のダイアログには tauri-plugin-dialog の追加が必要(現在の依存は store と opener だけ: `Cargo.toml:15,25`)。既定は OFF。
4. **段階 2: CloudKit で実装を進める**(オーナー決定 2026-10-14)。段階 1 の後に着手する。Apple Developer Program への加入と署名(Issue #9)が前提。調査結果は Issue #17 のコメントと Obsidian のメモ(`dot-CloudKitセーブ同期調査`)にある。段階 1 と同じデータモデル(端末別の台帳 + マージ規則)を使い、保存先だけを CloudKit に替える。

### 仕様の範囲外: 職場の端末のポリシー
- 職場の端末で個人のクラウドを使ってよいかは、仕様とは無関係な運用上の判断で、設計の前提にしない(オーナー決定 2026-10-14)。dot は指定されたフォルダを読み書きするだけで、同期サービスの可否には関与しない。
- 技術上の注意だけ残す: 同期サービスはファイルを遅延して伝え、退避(evict)や競合コピー(`pet-state 2.json` 等)を作りうる。書き込みの原子性と読み込み失敗時の退避(P1)で対処する。

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
- **GIVEN** P2 修正後の Adult で needs が 100、**WHEN** 600 分経過(起動中、5 秒ポーリング)、**THEN** hunger は 76±1(倍率 0.2)。同じ条件の Child では 100−120 で 0 になる。(R5/F4)
- **GIVEN** P2 修正後、**WHEN** 5 秒ごとに 720 回 update する、**THEN** hunger は 1 回の 3600 秒 update と ±1 で一致する。(F0)
- **GIVEN** P2 修正後の Adult で全ステータス 100、アイドル秒数を常に 0(在席)に差し替える、**WHEN** 5 秒ごとの update で 69 時間を進める、**THEN** 生存しており、hunger と happiness は 0、energy は 17±1、health は 5±1。さらに 3 時間(合計 72 時間)進めると死亡している(T_death ≈ 70.8 時間)。(F0 の逐次積分 / F4。Revision 2: 旧版の「閉じたまま 41 時間」は不在になったので、在席で検証する。倍率は 0.2)
- **GIVEN** Sleeping(残り 1 時間)の状態で、アイドル秒数を 0 に差し替える、**WHEN** 5 秒ごとの update で 3 時間を進める、**THEN** 最初の 1 時間は睡眠中の規則(energy +0.5/分、happiness は減らない)、残りの 2 時間は起きている規則で計算した値と ±1 で一致する。(F0)
- **GIVEN** P6-a 適用後、倍率 1.0(現行の全ステージ)、全ステータス 100、**WHEN** `last_update` を 63 時間前にして update する、**THEN** 生存しており、値は 30/95/100/100(±1)。(F5 / 週末)
- **GIVEN** 同じ条件で Adult(倍率 0.2、R5 適用後)、出発時 70/70/50/100、**WHEN** 63 時間・216 時間(9 日)・720 時間(30 日)前を `last_update` にしてそれぞれ update する、**THEN** 3 つとも生存しており、値は 30/69/100/100(±1)。(F5)
- **GIVEN** 出発時 60/5/60/100、**WHEN** 30 日前を `last_update` にして update する、**THEN** 生存しており、health は 98 以上。(F5 の health の保護)
- **GIVEN** 出発時 hunger 15、**WHEN** 24 時間前を `last_update` にして update する、**THEN** hunger は 15 − 在席の規則 30 分ぶん(倍率 1.0 なら 6、Adult なら 1.2)の値から下がっていない。(F5 の下限)
- **GIVEN** 29 分前を `last_update` にする、**WHEN** update する、**THEN** 29 分すべてを在席の規則で計算した値と一致する(不在区間は 0)。(F5 の閾値)
- **GIVEN** P6-b 適用後、アプリは起動中、アイドル秒数を 2 時間に差し替える、**WHEN** 60 秒の tick が走る、**THEN** その 60 秒は深い眠りの規則で計算される。アイドル秒数が 10 分なら、在席の規則で計算される。(F5 の離席)
- **GIVEN** P6 適用後の Adult、全ステータス 100 で月曜 9:00 に開始し、平日 9 時間は在席(アイドル 0)、それ以外は不在、世話をしない、**WHEN** シミュレーションを進める、**THEN** 7 営業日目に health が 0 になって死亡する。死亡までの在席時間は、予定の在席時間だけで数えて 56〜59 時間、不在の最初の 30 分を含めて 59〜62 時間の範囲に入る。不在区間(最初の 30 分を除く)では死亡しない。(F4 の表。境界の誤差を避けるため範囲で判定する)
- **GIVEN** P6 適用後の Adult(倍率 0.2)、同じ条件で、毎日 9:30 に FEED、14:00 に PLAY をする、**WHEN** 130 日分を進める、**THEN** 生存している。(F4 の世話の表 / 決定事項 18)
- **GIVEN** 同じ条件で倍率だけ 0.4 にする、**WHEN** 進める、**THEN** 9 営業日目に死亡する(0.4 を採らない根拠の回帰テスト)。(F4)
- **GIVEN** age が X の Child、**WHEN** 20 日前を `last_update` にして update する、**THEN** age は X + 14 日(AGE_OFFLINE_CAP)。時計を戻した場合は X のまま。(F5 の age)
- **GIVEN** B=0 で、その日に開く・FEED 2 回(hunger < 80)・フォーカス完走 1 回、**WHEN** 日付が変わる、**THEN** B=31。(F1)
- **GIVEN** 同じ日に、FEED を hunger ≥ 80 で 3 回と hunger < 80 で 7 回行い、フォーカスを 3 回完走して開いた、**WHEN** 日付が変わる、**THEN** G=min(50, 10+15+30)=50。(F1)
- **GIVEN** B=280 で、最後に開いた日から 5 日後(m=5)、アプリは閉じていた、**WHEN** 起動して日次確定する、**THEN** B=264 であり、100 を下回らない。(F1、GRACE=3)
- **GIVEN** 同じ条件でアプリは d1–d5 の間ずっと起動していた(毎日確定が走る)、**WHEN** d5 の確定後、**THEN** B=264(閉じていた場合と同じ値)。(F1 の差分方式)
- **GIVEN** B=280 で金曜に開き、土日は開かない、**WHEN** 月曜(m=3)に日次確定する、**THEN** B=280(週末では減らない)。(F1、GRACE=3)
- **GIVEN** B=105(ティア 100 に到達済み)で 30 日未訪問、**WHEN** 確定する、**THEN** B=100。(F1 のティア下限)
- **GIVEN** 時計を 2 日戻す、**WHEN** update が走る、**THEN** age・needs・B・当日カウンタはいずれも変化しない。(Edge Cases)
- **GIVEN** 静寂時間 22–08 で 23:00 に 10 分離席して戻る、**WHEN** 戻る、**THEN** トレイの「話したい」表情は出ない。(F3)
- **GIVEN** ActiveFull で、08:00 から 1 時間ごとに 15 分(IDLE_AWAY_MIN 以上)離席して復帰する、**WHEN** 22:00 になる、**THEN** 「話したい」表情は 5 回以下で、各回の間隔は 45 分以上。(F3)
- **GIVEN** ActiveFull で QUIET 1H を押した、**WHEN** 15 分の離席から復帰する、**THEN** 「話したい」表情は出ない。60 分後には再び出せる。(F3 / muted)
- **GIVEN** ActiveFull で Mood が `calm` から `cheerful` に変わる条件が 3 分ごとに入れ替わる、**WHEN** ポップオーバーを開いたまま 30 分経つ、**THEN** 表情の切り替えは 3 回以下(MOOD_MIN_DWELL=600 秒)。(F2)
- **GIVEN** ActiveFull で「話したい」表情が出ている、**WHEN** トレイをクリックして開く、**THEN** トレイは通常の表情に戻り、tooltip は `dot — <mood>` になる。(R9)
- **GIVEN** Onboarding で LATER を押した、**WHEN** 7 日後に開く、**THEN** `STAY WITH YOU?` がもう一度だけ表示される。そこでも LATER を押すと、以降は表示されない。(R1 / States)
- **GIVEN** Adult が Idle で Sleeping 中、**WHEN** 開く、**THEN** status は `sleeping (Xh Ym left)` のまま表示され、Mood 名では上書きされない。(UI)
- **GIVEN** Adult で全ステータス 100、アイドル秒数を 0(在席)に差し替えて 72 時間世話をしない、**WHEN** ポップオーバーを開く、**THEN** `THANK YOU.` と `NEW FRIEND?` が表示され、コインと実績は保たれる。Standard では MEMORIES がアルバムに残る。(R10/F4。Revision 2: 閉じていた 72 時間では死なない)
- **GIVEN** Companion モジュールの全公開関数(Bond 確定、フォーカス完走、台詞選択、オンボーディング)、**WHEN** 単体テストで各関数の前後の `GameStats.coins` と `total_coins_earned` を比べる、**THEN** どちらも変化しない。Companion のコードから `add_coins` / `try_spend` への呼び出しは静的にも 0 件。(R11/A7)
- **GIVEN** `companion.json` に不正な JSON、**WHEN** 起動する、**THEN** `companion.corrupt-*.json` に退避して Companion だけをデフォルトで作り直す。pet-state.json と game-stats.json は変わらない。(Edge Cases/P1)
- **GIVEN** 既存の v1.0.3 の Adult のセーブ、**WHEN** 新しいバージョンで起動する、**THEN** ペットのステータスは変わらず、B=0 の companion.json が作られる。(Edge Cases)
- **GIVEN** ポップオーバーを閉じて 10 分、**WHEN** Activity Monitor で計測する、**THEN** 平均 CPU は 0.1% 未満で、TCC のダイアログは 1 回も出ていない。(P3/プライバシー)
- **GIVEN** フォーカスを 4 分で中止、**WHEN** 日付が変わる、**THEN** n_focus は 0 で、ペナルティはない。(R8)

**複数端末(P7、段階 0 の単体テスト。2 つのフォルダで 2 台を模擬する)**
- **GIVEN** 端末 A と B が、時間帯の区切りに揃った同じ 2 時間(k0 から k0+11 までの 12 区間)にそれぞれ支払いを記録した、**WHEN** マージする、**THEN** パッシブ収入は 12 枚(2 時間 ÷ 10 分)で、24 枚にならない。(二重加算の防止)
- **GIVEN** A で FEED 3 回、B で FEED 2 回(どちらも別のファイルに記録)、**WHEN** マージする、**THEN** total_feeds は 5。同じファイルを 2 回マージしても 5 のまま。(G-Counter / 冪等)
- **GIVEN** A のスナップショットは 10:00 に FEED、B は 9:00 に FEED したあと 5 秒ごとに `last_update` だけが更新されている、**WHEN** マージする、**THEN** A のスナップショットが採られる。(世話の時刻で比べる)
- **GIVEN** A の device ファイルが壊れた JSON、**WHEN** マージする、**THEN** A は飛ばされ、前回のマージ結果と B から状態が作られる。新しいペットにはならない。(P1 と同じ方針)
- **GIVEN** `devices/<A>.json` と、それより古い版の競合コピー `devices/<A> 2.json`、**WHEN** マージする、**THEN** 結果は `<A>.json` だけをマージした場合と同じで、カウンタは二重にならない。競合コピーは `conflicts/` に移る。
- **GIVEN** A と B の同じ日の当日カウンタが、それぞれ g=1・n_care=5・n_focus=2、**WHEN** マージして日次確定する、**THEN** その日の G は 50(G_CAP)で、100 にならない。g は 1 として数える。(Bond の日次上限)
- **GIVEN** A と B に別々の id の生きたペットがいて、コインが A 40 枚・B 25 枚、**WHEN** 同期を初めて有効にしてマージする、**THEN** `WHICH DOT?` が表示され、選ぶまではどちらのペットも変わらない。選んだあとは選ばれたペットだけが残り、もう一方はアルバム(MVP ではアーカイブファイル)に移る。コインは 65 枚になる。(初回のマージ)

## Open Questions

1. ストリークの日付(UTC: `stats.rs:117`)と Companion の日付(ローカル)を揃えるか。推奨: 両方ローカルに統一する(既存のストリークを移行する時に 1 日分を救済する)。
2. 進化を日単位に延ばす TODO と Companion を同じリリースで出すか。推奨: 別にする。Companion を先に出す(日単位化は既存ユーザーの Child を巻き戻すかどうかの判断が必要)。
3. P2 の修正で Baby/Child の実効的な減衰が急に強まる(起動中も減るようになる)。この再調整を Companion のブランチに含めるか。推奨: P2 を独立した PR にして先に出す。
4. 台詞のローカライズ(日本語)を行うか。推奨: 英語の大文字に統一したまま(UI 全体が英語のため)。
5. (Revision 2)現行のストリークは、世話をしない日が 1 日でもあると 1 に戻る(`stats.rs:131-137`)。平日だけ使う人は、毎週月曜にストリークが切れ、実績 `streak_7` / `streak_30`(`achievements.rs:49-58`)を取れない。アンチピラー 5「連続ログインを罰しない」とも緊張がある。推奨: 「2 日までの空白は連続とみなす」に変える(Companion とは別の PR)。日付をローカルに揃える Q1 と同時に行う。
6. (Revision 2)iCloud の evict されたファイルを普通の読み込みで読んだときの挙動、一時ファイルを同期クライアントがアップロードするかどうか、アイドル秒数がシステムのスリープ時間を含むかどうかは、いずれも未確認。P6-b と P7 の実装前に実機で確かめる。
7. (Revision 2)WKWebView のタイマーが、ポップオーバーを隠している間も 5 秒ごとに動き続けるかどうかは未確認。動かない場合、隠している間は update の空白が長くなり、空白による不在の判定(30 分超)に入ることがある。P3 で Rust の tick に移せばこの問題はなくなる。

---

## レビュアーへの決定事項

> **承認済み(2026-10-14)**: 項目 1–10 は、オーナーが原則すべて推奨案で承認した。ただし項目 7(大人期の死)は、オーナーの指示で修正した(下記)。項目 11 以降は Revision 2 で追加したもの。11–17 は PR #3 のマージ(2026-10-14)をもって承認とみなし、**18 はオーナーが明示的に承認(0.2、2026-10-14)**した。

1. **MVP の範囲** — 推奨: 「最小(MVP)」案を初回リリースにし、標準を到達点にする。理由: 前提タスク P1–P3 自体が大きく、まず邪魔にならないことを実地で確かめたい。権限ゼロ・経済の接続ゼロで、失敗しても戻せる。
2. **権限を要する連携の可否** — 推奨: 打鍵監視と Focus 検知は採用しない。カレンダーと通知は野心的な案のオプトインまで凍結する。MVP と標準は TCC のプロンプト 0 回。理由: 未署名の配布ではリリースごとに許可が外れる(TD)。キーロガーに見えるリスクがあり、ピラー P1 とアンチピラー 4 に反する。アイドル時間は権限なしで在席を推定できる。
3. **コイン経済との接続(ファウセット)** — 推奨: Companion からコインを一切出さない(フォーカスも Bond のみ)。理由: 現状ですでにカジュアルで 1 日約 +50 枚、ヘビーで約 +190 枚の余剰がある(economy-designer の試算)。活動報酬は自動クリック等で稼げてしまう。
4. **コイン経済との接続(シンク)** — 推奨: 標準の段階で、Bond ティアで解放する白黒アクセサリを最大 4 種(40/80/150/300 枚、ステータス効果なし、1 枠)だけ導入するかを、MVP の反応を見て判断する。MVP では導入しない。理由: 余剰を吸うシンクは必要(economy)。一方、コスメショップはミニマルな美学を壊す(creative)。両者の妥協として、数量を絞り、絆で解放する形にした。
5. **既存経済の穴(P4)** — 推奨: Companion とは別の PR で、ゲーム報酬のサーバー側検証と日次上限 30 枚を先に入れる。理由: クライアントの申告を信じる現状では、どんな価格設計も無意味になる。
6. **通知頻度の既定値** — 推奨: OS 通知は作らない。トレイの「話したい」表情は 1 日最大 5 回・最小間隔 45 分・静寂時間 22–08。ポップオーバー内の台詞は 1 回開くごとに最大 1 行・同じカテゴリは 24 時間に 1 回。理由: CLAUDE.md の「calm and ambient」。ux 案と systems 案のうち保守的な方を採った。
7. **大人期の死**(**修正して承認**) — 決定: **大人期の死は、在席中の完全放置でのみ起きる。不在時間では起きない**。在席 = アプリが起動中で、Mac が起きていて、最後の入力から AWAY_THRESHOLD 以内。死ぬには、世話をしないまま在席時間が合計で約 46〜61 時間たまる必要がある(1 日 4〜9 時間の在席で 7〜12 営業日。倍率 0.2: F4)。演出は「別れ」(`THANK YOU.` / `NEW FRIEND?`)のまま。床値を設けて完全に死なない案は、引き続き採らない。
   - 修正の理由: オーナーは職場の Mac で使い、週末は世話ができない。旧案(閉じている間も減衰し、約 42 時間で死ぬ)では、週末(63 時間)ごとに別れが起きる。
   - オーナー案の「起動中の完全放置」から、条件を「在席中」に狭めた。職場の Mac をつけっぱなしにした週末でも死なないようにするため(§不在時間の扱い)。
   - 旧推奨: 死はありうる(約 42 時間の完全放置)。緊張がゼロになると世話の意味が消えるので、床値は設けない。
8. **復活時の絆** — 推奨: 新しいペットは B=0 から始め、前のペットの MEMORIES はアルバムに永久保存する。理由: 新しいペットは新しい関係(CD・economy の一致した意見)。systems 案の 25% 引き継ぎは、死を軽くしすぎる。
9. **絆の可視化** — 推奨: 数値は出さず、表情・台詞・MEMORIES で間接的に伝える。理由: ゲージを見せると最適化の対象になり、P3「時間が関係を育てる」が崩れる。
10. **前提タスク P1(セーブ消失)と P2(減衰の切り捨て)の扱い** — 推奨: Companion より先に、独立した修正 PR として出す。理由: P1 はデータ消失(重大度: 高)、P2 は大人期の減衰倍率の前提になる。どちらも Companion がなくても必要な修正。

### Revision 2 で追加した決定事項(承認済み 2026-10-14)

11. **不在時間の扱い** — 推奨: 不在時間は「深い眠り」として扱う。睡眠の規則(hunger は半分の速さ、happiness は減らない、energy は回復)に、hunger の下限 30 と「health を減らさない」を加える(F5)。全ステージに適用し、Companion より先に P6(Issue #16)として出す。
    - 理由: 睡眠の規則だけでは、計算上、現行の全ステージ(倍率 1.0)は典型的な金曜の状態から 42.8 時間で死に、週末を越えられない。Adult(0.2)も 9 日の連休は越えられず、happiness < 10 なら 33.3 時間で死ぬ。下限と health の保護を加えると、63 時間・9 日・30 日のすべてで生存する。「待っていてくれる」の核とも合う。
    - 次点: 完全凍結(c)。実装は最も簡単。
    - 採らない: 減衰の上限(a)。出発時の状態によっては、上限 36 時間でも 20.9 時間で死ぬ。曜日スケジュール(b)。平日の夜・病欠・連休を守れない。
12. **AWAY_THRESHOLD の既定値** — 推奨: 30 分(安全範囲 10–60)。理由: 休憩や短い離席より長く、会議 1 本より短い。不在の判定には、空白(アプリ非起動・スリープ)と離席(HID アイドル、権限不要)の両方を使う。最初の 30 分は在席の規則で数えるので、29 分と 31 分の間で結果が大きく変わることはない。
13. **不在中の成長(age)と OFFLINE_CAP** — 推奨: age は不在中も進める。空白 1 回の上限は 14 日(AGE_OFFLINE_CAP)。needs の OFFLINE_CAP は 72 → 96 時間に延ばして残す(F5 の収束条件を、ノブの安全範囲の端でも満たすため。結果は変わらない)。理由: 眠っている間も育つ。進化を日単位にする Issue #10 では、カレンダーの日数で育つほうが「旅」になる。#10 を再開するときに上限を見直す。
14. **Bond の猶予(GRACE)** — 推奨: 2 日 → 3 日。理由: 金曜に開いて月曜に開くと m=3 になる。GRACE=2 のままだと、週末のたびに Bond が 8 ずつ減り、「不在を責めない」(Player Fantasy)に反する。
15. **同期の段階導入**(**オーナー決定 2026-10-14 で更新**)— 段階 1: ユーザーが指定するフォルダにペットのファイルを置く / 段階 2: CloudKit で実装を進める。順序は P1(Issue #11)→ 段階 0(端末別ファイルとマージを、同期サービスなしで作って単体テストする)→ 段階 1 → 段階 2。既定は OFF。理由: P1 なしで同期を入れると、同期中の不完全なファイルを読んだだけでペットとコインを失う。
16. **同期の方式** — 推奨: 端末ごとのファイル(`devices/<device_id>.json`)に各端末が自分の分だけを書き、読むときに可換・冪等な規則でマージする。PetState は「最後に世話をした時刻」で選ぶ。カウンタ・コイン・実績・Bond の当日カウンタ・MEMORIES は、端末別の台帳から加算か和集合で作る。パッシブ収入は時間帯の和集合で数える。理由: 1 ファイルを共有して上書きすると、現行コードでは他端末の変更が消える。素朴に加算すると、パッシブ収入が二重に払われる(§複数端末)。
17. **職場の端末での利用可否** — **削除(オーナー決定 2026-10-14)**。職場の端末のポリシーは仕様と無関係で、設計の前提にしない。段階 1 は dot の仕様として進める。
18. **大人期の減衰倍率(ADULT_DECAY_MULT)** — 推奨: 0.4 → 0.2(安全範囲 0.15–0.2)。Review 2 の指摘 #1 による。
    - 理由: 0.4 では、1 日 9 時間在席して毎日 FEED 1 回 + PLAY 1 回をしても、9 営業日目に死ぬ。不在中は hunger も happiness も回復しないので、在席中の消費を 1 日の世話で返しきれない。0.2 なら同じ世話で 130 日以上生存する。世話をしなければ 7〜12 営業日目に死ぬ(F4 の表)。
    - 「世話を義務から挨拶へ」(Overview)と、決定事項 7 の「完全放置でのみ死ぬ」を満たすのは 0.2 以下だけ。
    - 承認済みの MVP の範囲(決定事項 1)の中で、R5 の値だけを変える。
    - 代替案: 0.4 のまま、深い眠りで happiness を少しずつ回復させる案。物語には合うが、hunger の不足(1 日 45.6 の消費に対して FEED 1 回で +30)は解決しない。

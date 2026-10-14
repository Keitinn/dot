# Design Review Log — companion.md

## Review 1 — 2026-10-14

- **Reviewer**: `/design-review design/gdd/companion.md` を、作成セッションとは別の独立サブエージェント(general-purpose)内で実行した。作成時の文脈は引き継いでいない。
- **Mode**: `review_mode: solo` / `workflow: minimal`(CCGS 設定)。solo のため、スキルの規定で専門家の委任(Phase 3b)とレビューログ追記(Phase 5)は省略された。このログは作成者側で記録したもの。
- **Verdict**: **NEEDS REVISION**
- **規模**: L(複数システム、式 4 本、新しいセーブファイル)
- **照合結果**: コード参照の行番号はすべて正確。F4 の算術(Adult 41.7h / Child 16.7h)は一致。依存先 GDD は存在しないが、GDD 側で明記しているためブロッキングにはしなかった。

### Findings と対応

| # | 重大度 | 節 | 指摘 | 対応 |
|---|---|---|---|---|
| 1 | blocking | Edge Cases / F0 / AC | DT_CAP=6h だと、アプリを閉じている間は大人が死ねず、AC(43h 放置で死亡)と矛盾する。age への適用も不明 | DT_CAP を廃止。代わりに OFFLINE_CAP=72h(needs と age の両方に適用、時計を戻した場合は 0)を置いた。Tuning Knobs に追加 |
| 2 | blocking | F0 / F4 / AC | 一括計算では health 判定が区間末の状態で 1 回だけ行われるため、死亡は約 33h になり AC「41h 生存」が落ちる。睡眠の途中終了も「起きていた」扱いになる | P2 を「1 分刻みの逐次積分(最大 4320 ステップ)。睡眠終了をまたぐ区間は分割する」と明記。F4 はこの前提での値と注記 |
| 3 | blocking | F1 | Bond の減衰が、起動状態によって二重に数えられる。どの日の G を加えるのかも不明 | D を差分方式(`overdue_now − overdue_applied`)で再定義。G は「前回の確定以降に貯まった当日カウンタ」を、次の確定で加える。連続起動の AC を追加 |
| 4 | recommended | R3 / Edge Cases | 台詞カテゴリの優先順位が不完全で、0 行になる条件もない | 全カテゴリの優先順位を定義。低優先カテゴリには LINE_SPEAK_P(シード可能な乱数)を導入 |
| 5 | recommended | 導入計画 / F1 | MVP には Focus がないので、ティア到達日数が合わない。MVP の R10 が Standard 機能(アルバム)を参照している | MVP / Standard 別の到達日数表を追加。アルバムは Standard と明記 |
| 6 | recommended | Interactions | 当日の世話回数は GameStats にない | Companion 側(`companion.json`)で当日カウンタを持つと明記 |
| 7 | recommended | Tuning Knobs | 閾値の掲載漏れ。移行時の猶予 3 日が GRACE と食い違う | 掲載漏れを追加。移行時は last_open_date=今日 として GRACE に一本化 |
| 8 | recommended | States | LATER から再度 Onboarding へ戻る遷移がない。Muted と Focus の重なりが不明 | 状態とフラグ(muted)に分離。再提示の遷移を追加 |
| 9 | recommended | UI | status を Mood に置き換えると sleeping 等の表示が消える | 行動中(Sleeping/Eating/Playing)の表示を優先し、Idle のときだけ Mood を表示 |
| 10 | recommended | Interactions | 実績 Kindred は見せない絆を露出する。check_achievements の引数変更も必要 | Kindred を廃止し、「大人期で 30 日一緒」(`Old Friends`)に置き換え。インターフェース変更を明記 |
| 11 | recommended | AC | コイン AC が判定不能。F3 の離席条件、DWELL・LATER・QUIET・トレイの AC がない | 単体テストの形に書き換え、不足分の AC を追加 |
| 12 | recommended | Header | 存在しないレビューログを参照している | 本ログを作成し、Status を更新 |
| 13 | recommended | F2 | DWELL の適用範囲が曖昧 | 「開いた瞬間は毎回再計算、DWELL は表示中とトレイの更新だけに適用」と定義 |
| 14 | nit | P4 | 1 回の最大獲得は 10 枚(memory) | 修正 |
| 15 | nit | F3 | LINE_* は R3 の変数 | R3 と Tuning Knobs に移動 |
| 16 | nit | P3 | tick 移行時は accrue_passive も移す必要がある | P3 に追記 |

### 未実施事項
- 専門家エージェントによるレビュー(full モード)は実施していない。必要なら `/design-review design/gdd/companion.md --review full` を新しいセッションで実行する。
- 修正後の再レビューは未実施。

# Changelog

## Unreleased

### Added
- 大人期コンパニオンの土台: 絆(Bond)を `companion.json` に保存し、1 日 1 回確定する(UI にはまだ出ない)
- 大人期コンパニオンの気分と台詞: ポップオーバーを開いたときの Mood と一言(全 68 本)を Rust 側で決めるコマンドを追加(UI にはまだ出ない)
- 大人期コンパニオンの UI: 開いたときの目の表情 5 種と一言の吹き出し、`STAY WITH YOU?`、`*` メニューの `COMPANION` 設定(ON/OFF・静寂時間・QUIET)
- 大人期の別れ: 大人で死んだときは `GAME OVER` の代わりに、目を閉じた dot と `THANK YOU.`、`NEW FRIEND?` を出す(子どもまでは従来どおり)

### Fixed
- 不在時間(アプリ非起動・スリープ)で死なないようにした
- 起動したまま 30 分以上操作がない時間も不在として扱うようにした(つけっぱなしの Mac でも週末に死なない)
- 起動中に hunger・happiness・energy がほとんど減らなかった問題を直した(端数を切り捨てずに持ち越す)
- 大人になると hunger・happiness・energy の減り方が 5 分の 1 になるようにした(1 日 FEED 1 回 + PLAY 1 回で暮らせる)
- ポップオーバーを閉じている間は描画と 5 秒ごとの更新を止め、ペットの更新とコインのパッシブ収入は Rust 側で 60 秒ごとに続けるようにした

## v1.1.0

### Added
- Mini games: janken, memory, and blackjack. Playing never costs coins; wins pay out coins
- Coins: earned from mini games, passively (1 coin every 10 minutes while the pet is alive), and as a daily care streak bonus (1 coin per streak day, up to 10). Coins carry over when the pet is revived
- Snack shop: buy candy, cookie, cake, coffee, or medicine with coins. Snacks are independent of the FEED button and its cooldown

## v1.0.3

### Added
- Sleep mechanic: sleeping lasts 1-4 hours with gradual energy recovery
- Cooldowns: FEED 30 minutes, PLAY 20 minutes
- Status line showing what dot is doing
- Version label in the settings menu

## v1.0.2

### Changed
- Hide the Dock icon (menu bar only, `LSUIElement` via `Info.plist`)
- Tone down pet animations: slower, more subtle movements

## v1.0.1

### Changed
- Hide the app from the Dock (`LSUIElement=true`)
- Pin Tauri npm packages to match the Rust crate versions

## v1.0.0

### Added
- Streaks: daily care streak tracking displayed in pet info line
- Achievements: 10 unlockable achievements (evolution milestones, care streaks, action counters, revival)
- Achievements panel: trophy button (+) opens a full overlay showing all badges
- Achievement toast: brief notification when a new achievement is unlocked
- Game stats persistence: lifetime counters saved separately from pet state (`~/Library/Application Support/dot/game-stats.json`)
- Unit tests: 57 tests covering needs decay, evolution, lifecycle, state actions, stats, and achievements
- CI: GitHub Actions workflow for testing and building on push/PR
- Release: GitHub Actions workflow for building and publishing `.dmg` on version tags
- MIT LICENSE file

### Changed
- Action commands (feed, play, sleep, revive) now return pet state along with any newly unlocked achievements
- Trimmed `tokio` features from `full` to `rt, macros`
- Bumped version to 1.0.0
- Updated metadata in Cargo.toml, tauri.conf.json, and package.json
- Added `category: entertainment` to macOS bundle config

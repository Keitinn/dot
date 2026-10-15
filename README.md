# dot

[![CI](https://github.com/Keitinn/dot/actions/workflows/ci.yml/badge.svg)](https://github.com/Keitinn/dot/actions/workflows/ci.yml)

A cute Tamagotchi-style virtual pet that lives in your macOS menu bar.

This is a fork of [tooyipjee/dot](https://github.com/tooyipjee/dot) (MIT, Copyright (c) 2025 Jason Too).

## Features

- **Menu Bar Pet** - Lives quietly in your menu bar, always accessible
- **Pet Care** - Feed, play with, and put your pet to sleep
- **Evolution** - Watch your pet grow from Egg → Baby → Child → Adult
- **Stats System** - Monitor hunger, happiness, energy, and health
- **Streaks & Achievements** - Track daily care streaks and unlock 10 achievements
- **Persistence** - Your pet and stats survive app restarts
- **Death & Revival** - If neglected, your pet may pass away, but you can start fresh

## Install

### Homebrew (macOS)

```bash
brew tap tooyipjee/dot
brew install --cask dot
```

### Download

Grab the latest `.dmg` from [Releases](https://github.com/Keitinn/dot/releases).

### Gatekeeper bypass

Releases are not code-signed yet (setup: [design/signing-setup.md](design/signing-setup.md)), so macOS will show a warning on first launch. To fix:

```bash
xattr -cr /Applications/dot.app
```

Or right-click the app → Open → confirm.

### Build from source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/Keitinn/dot.git
cd dot
npm install
npm run build
```

## Usage

- **Click the menu bar icon** to toggle the pet window
- **FEED / PLAY / SLEEP** buttons to care for your pet
- **+** button to view achievements
- **\*** button for settings (reset / quit)

## Achievements

| Achievement | How to unlock |
|------------|--------------|
| First Hatch | Evolve from Egg to Baby |
| Growing Up | Reach Child stage |
| All Grown Up | Reach Adult stage |
| Caretaker | 3-day care streak |
| Devoted | 7-day care streak |
| Best Friend | 30-day care streak |
| Gourmet | Feed 100 times |
| Playmate | Play 50 times |
| Lullaby | Put to sleep 50 times |
| Second Chance | Revive a pet |

## Tech Stack

- **Frontend**: Vanilla JavaScript, HTML5 Canvas
- **Backend**: Rust with Tauri v2
- **Persistence**: JSON file storage (`~/Library/Application Support/dot/`)

## License

MIT

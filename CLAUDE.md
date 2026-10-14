# dot - Project Notes

## Overview
A Tamagotchi-style menu bar pet for macOS. Minimalist black & white pixel art aesthetic.

---

## TODO / Future Work

### Animation
- [x] Current animations are **too vigorous** - tone them down (done in v1.0.2)
- Make movements subtle and gentle, not hyperactive
- Should feel calm and ambient, not distracting

### Evolution Duration
- [ ] Phases should be measured in **days**, not hours
- Suggested timeline:
  - Egg → Baby: 1-2 days
  - Baby → Child: 3-5 days
  - Child → Adult: 1 week+
- The journey should feel meaningful and rewarding

### Adult Phase - Companion Feature
- [ ] When pet reaches Adult stage, introduce **companion functionality**
- Ideas to explore:
  - Reacts to user activity (typing, idle, meetings?)
  - Shows different moods based on time of day
  - Occasional ambient animations/expressions
  - Maybe integrates with system events (low battery, calendar, etc?)
  - Could show encouraging messages or gentle reminders
  - Becomes a "work buddy" that acknowledges your presence
- Goal: Transform from "pet to care for" into "companion that cares for you"

---

## Design Principles
- Minimalist black & white pixel art
- Tamagotchi-inspired but modern
- Non-intrusive menu bar presence
- Simple, clear iconography (cute pixel eyes)
- Lightweight and performant

---

## Tech Stack
- Tauri v2 (Rust backend)
- Vanilla JS frontend
- macOS menu bar integration

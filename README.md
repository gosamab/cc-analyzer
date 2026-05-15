# cc-analyzer

Local-first desktop app for analyzing Claude Code usage. Reads `~/.claude/projects/**/*.jsonl`, shows cost/utilization breakdowns, and surfaces actionable recommendations. Fully offline — no network calls.

## Stack
- **Shell:** Tauri 2 (Rust backend, web UI)
- **UI:** Svelte 5 + TypeScript + Tailwind + uPlot
- **Storage:** SQLite cache at `~/Library/Application Support/cc-analyzer/cache.db`
- **Parsing:** Rust, streaming JSONL, incremental (only new lines on re-launch)

## Modes
- **Launch-only** (default): parse on open
- **Menu bar agent** (opt-in): file-watch live, fire advisory `/clear` notifications

## Setup
```bash
cd ~/cc-analyzer
pnpm install
cd src-tauri && cargo build && cd ..
pnpm tauri dev
```

## Project layout
```
src/                 # Svelte frontend
  lib/views/         # Dashboard, Explorer, Insights, Settings
  lib/components/    # Shared UI
  lib/ipc.ts         # Typed wrapper around Tauri invoke
src-tauri/src/
  main.rs            # Entry
  parser.rs          # JSONL streaming parser
  db.rs              # SQLite schema + queries
  analyze.rs         # Aggregations + recommendations engine
  pricing.rs         # Model price table
  commands.rs        # Tauri command surface
```

## Status
Skeleton only. See `DESIGN.md` for the full design doc and milestones M1–M5.

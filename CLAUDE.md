# cc-analyzer — agent guide

Local-first desktop app that analyzes Claude Code usage. Reads `~/.claude/projects/**/*.jsonl`, surfaces token/cost/utilization breakdowns and actionable recommendations. Fully offline.

## Stack
- **Shell**: Tauri 2 (Rust + web UI), ~10MB binary
- **Frontend**: Svelte 5 (runes) + TypeScript + Tailwind + uPlot (planned, not yet used)
- **Storage**: SQLite via `rusqlite` at `~/Library/Application Support/cc-analyzer/cache.db`
- **Parsing**: Streaming JSONL with byte-offset checkpoints per file (incremental on relaunch)

## Run / build
```bash
pnpm install
pnpm tauri dev          # dev with hot reload
pnpm build              # frontend only
cd src-tauri && cargo build
pnpm tauri build        # packaged .app (regenerates icons too)
```

## File layout
```
src/
  App.svelte              tab shell, refresh button, lazy-mount-keep-alive views
  main.ts, app.css
  lib/
    ipc.ts                typed wrappers around the 8 Tauri commands
    format.ts             fmtUsd/fmtTok/fmtInt/fmtPct/fmtDec, shortProject, isoRange
    store.svelte.ts       UIState (view, range, selectedSession, projectFilter)
    components/
      RangePicker.svelte  1d / 7d / 30d / 90d
      Bar.svelte          horizontal token bar
      Sparkline.svelte    SVG sparkline (filled area + line)
    views/
      Dashboard.svelte    headline + KPI sparklines + by project/model + Gantt + top recs
      Explorer.svelte     horizontal filter bar + sessions list + session detail
      Insights.svelte     recs with concrete actions + deep-link buttons + "What's healthy" grid
      Settings.svelte     placeholder (M4)
src-tauri/src/
  main.rs, lib.rs         entry; manages AppState { db: Mutex<Db> }
  pricing.rs              price_for(model) → per-1M token rates (Opus/Sonnet/Haiku)
  db.rs                   schema, open(), file_offsets table for incremental reads
  parser.rs               refresh() walks ~/.claude/projects/**/*.jsonl, ingest_file()
  analyze.rs              summary, daily_breakdown, sessions, session_detail, utilization, recommendations, health_signals
  commands.rs             8 #[tauri::command] wrappers
src-tauri/
  Cargo.toml, build.rs, tauri.conf.json
  capabilities/default.json
  icons/                  generated via `pnpm tauri icon static/favicon.svg`
static/favicon.svg        1024×1024 with 100px safe-area margin (Apple HIG)
```

## Data shape (SQLite)
```sql
messages(id, session_id, project, ts, model,
         input_tok, output_tok, cache_w_tok, cache_r_tok,
         cost_usd, tools_json)
file_offsets(path, byte_off, mtime)         -- incremental ingest checkpoint
dismissed_recs(rec_key, dismissed_ts)
settings(key, value)
```

## IPC surface
All commands hang off `ipc.*` in `src/lib/ipc.ts`. Backend in `src-tauri/src/commands.rs`.
- `refresh_logs()` → new rows inserted
- `summary(since?, until?, project?)` → totals + by_model + by_project
- `daily_breakdown(since, until)` → per-day with nested sessions
- `sessions(since?, until?)` → top 500 sessions in range
- `session_detail(session_id)` → session-level token totals (in/out/cache_w/cache_r) + turns (each with cache_w/cache_r + per-turn `tools: [{name, count}]`) + top_files + tool_counts
- `recommendations(since, until)` → severity-prioritized list; each rec carries `action` text + optional `action_session_id` / `action_project` for Explorer deep-linking
- `health_signals(since, until)` → list of passing health checks (cache reuse, model mix, session discipline) rendered as the "What's healthy" grid
- `utilization(day)` → today's work blocks + hourly + ratios

All Rust types carry both `tokens_total` and `cost_usd`. **Tokens are the primary metric**; cost is secondary and muted in the UI. Recommendations carry both `estimated_savings_tokens` and `estimated_savings_usd`.

## UI conventions (load-bearing — don't violate)
- **Tokens primary, cost secondary**: tokens use `text-ink`, costs use `text-muted`.
- **All numbers** go through formatters from `lib/format.ts` (Intl.NumberFormat). No raw `toFixed` in views.
- **Numeric columns**: `shrink-0` + parent `gap-3`. Header widths must match row widths exactly.
- **Truncated text**: `min-w-0 flex-1 truncate` with `title={fullValue}` for hover.
- **Project paths** display via `shortProject()` → `…/CloudDocs/Accounting` (last 2 segments).
- **Model names**: strip `claude-` prefix in tight columns.
- **Dark theme only.** Color tokens in `tailwind.config.js`: `bg`, `panel`, `panel2`, `border`, `ink`, `muted`, `accent`, `ok`, `warn`, `err`.
- **Card pattern**: `<div class="card"><div class="card-title">…</div>…</div>`.

## Performance notes
- Views are lazy-mounted on first visit then kept alive via `class:hidden` (not `{#if}`), so tab switches don't re-run SQLite aggregations.
- Initial refresh on launch ingests only new bytes per file (byte-offset checkpoint).
- For ~35k messages, initial mount of a view takes < 1s.

## Design decisions (locked in)
- **Tauri** (not Electron / SwiftUI / Python web server).
- **Dashboard + Explorer + Insights + Settings**, dashboard as entry view.
- **Launch-only** by default; menu-bar agent + notifications planned as M5 opt-in.
- **Heuristic-only recommendations** (no API calls, fully offline).
- **Conservative notification posture** when M5 lands: only context-bloat + 5h-block-limit on by default.
- **Quota mode**: simple 75% / 90% block warnings, no active throttling.

## What's done
- M1 core: Rust parser, schema, Tauri shell.
- M2 Explorer: sessions list with horizontal filters (range, project, model, min tokens, min turns, search, sort). Session detail now has a **token composition** stacked bar (cache_r / cache_w / input / output) and a **per-turn stacked bar chart** with a floating hover popover showing per-turn cache r/w, in/out, cost, and the actual tool calls. Long sessions are bucketed (max 240 bars; bucket size shown in header).
- M3: 3 recommendation rules (opus-overuse, low-cache-reuse, sprawling-session). Each rec now carries a concrete next-action string plus optional session/project deep-links (Insights buttons call `ui.drillSession` / `ui.drillProject` to jump into Explorer). New `health_signals` command + "What's healthy" grid covering cache reuse, model mix, session focus, and sessions-per-task.
- Dashboard v2: headline strip, KPI sparklines, Gantt timeline grouped by project, hourly turn heatmap, inline top recs.

## What's next (in order of priority)
1. **Settings (M4)** — pricing-table editor, plan/quota config, custom log path.
2. **Background agent (M5)** — `notify-rs` watcher, trigger engine, advisory `/clear` notifications. See `DESIGN.md` for rules + guardrails.
3. **More rec rules** from the catalog in `DESIGN.md`: repeated-rereads, bash-overuse, git-status-spam, context-bloat.
4. **Performance**: covering index on `messages(ts, project, model)`; `summary_cache` table for memoized aggregations.

## Open questions
- Multi-machine sync (iCloud symlink vs strict-local)?
- CSV export per view?
- Treat resumed sessions (same id, new file) as one chat?

## When extending
- **Adding a new aggregation**: add the function in `analyze.rs`, the `#[tauri::command]` in `commands.rs`, the TS type and wrapper in `ipc.ts`, then consume in a view.
- **Adding a new recommendation rule**: extend `analyze::recommendations`. Always set both `estimated_savings_tokens` and `estimated_savings_usd` (0 is fine if not applicable). Use stable `key` for dismissal.
- **Editing pricing**: `src-tauri/src/pricing.rs`. Eventually move to `settings` table editable from UI.
- **Schema changes**: bump in `db.rs::init_schema`, add a migration `CREATE TABLE IF NOT EXISTS` or `ALTER TABLE`. There's no migration framework yet — keep it idempotent.

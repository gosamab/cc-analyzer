# cc-analyzer — agent guide

Local-first desktop app that analyzes Claude Code usage. Reads `~/.claude/projects/**/*.jsonl`, surfaces token/cost/utilization breakdowns and actionable recommendations. Fully offline.

**Status: 1.0.0 — feature-complete and locked in. No roadmap, no planned work.**

## Stack
- **Shell**: Tauri 2 (Rust + web UI), ~10MB binary
- **Frontend**: Svelte 5 (runes) + TypeScript + Tailwind
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
    ipc.ts                typed wrappers around the Tauri commands
    format.ts             fmtUsd/fmtTok/fmtInt/fmtPct/fmtDec, shortProject, isoRange
    store.svelte.ts       UIState (view, range, selectedSession, projectFilter)
    components/
      RangePicker.svelte  1d / 7d / 30d / 90d
      DatePicker.svelte   custom-range picker
      Bar.svelte          horizontal token bar
      Sparkline.svelte    SVG sparkline (filled area + line)
      Donut.svelte        SVG donut chart
      Hint.svelte         info tooltip
      palette.ts          shared color palette for charts
    views/
      Dashboard.svelte    headline + KPI sparklines + by project/model + Gantt + top recs
      Explorer.svelte     horizontal filter bar + sessions list + session detail
      Commands.svelte     bash/tool/skill/MCP/slash-command usage tables
      Insights.svelte     recs with concrete actions + deep-link buttons + "What's healthy" grid
      Settings.svelte     pricing-table editor, cache stats, block usage, custom log path
src-tauri/src/
  main.rs, lib.rs         entry; manages AppState { db: Mutex<Db> }
  pricing.rs              price_for(model) → per-1M token rates (Opus/Sonnet/Haiku)
  db.rs                   schema, open(), file_offsets table for incremental reads
  parser.rs               refresh() walks ~/.claude/projects/**/*.jsonl, ingest_file()
  analyze.rs              summary, daily_breakdown, sessions, session_detail, utilization,
                          recommendations, health_signals, tool/skill/MCP/slash/top-commands
  commands.rs             #[tauri::command] wrappers (20 commands)
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
settings(key, value)                         -- pricing overrides, custom log path, etc.
```

## IPC surface
All commands hang off `ipc.*` in `src/lib/ipc.ts`. Backend in `src-tauri/src/commands.rs`.

Aggregation / browsing:
- `refresh_logs()` → new rows inserted
- `summary(since?, until?, project?)` → totals + by_model + by_project
- `daily_breakdown(since, until)` → per-day with nested sessions
- `sessions(since?, until?)` → top 500 sessions in range
- `session_detail(session_id)` → token totals + turns (per-turn cache r/w, in/out, `tools: [{name, count}]`) + top_files + tool_counts
- `utilization(day)` → today's work blocks + hourly + ratios
- `block_usage()` → current 5h block tokens / cost / window

Commands tab:
- `top_commands(since, until, limit)` → most-run bash commands with categories
- `tool_usage(since, until)` → built-in tool counts
- `skill_usage(since, until)` → Skill tool invocations grouped by skill name
- `mcp_usage(since, until)` → MCP tool calls grouped by server/tool
- `slash_command_usage(since, until)` → slash-command invocations

Insights tab:
- `recommendations(since, until)` → severity-prioritized list; each rec carries `action` text + optional `action_session_id` / `action_project` for Explorer deep-linking
- `health_signals(since, until)` → list of passing health checks rendered as the "What's healthy" grid

Settings tab:
- `cache_stats()`, `clear_cache()` — DB size + nuke button
- `pricing_table()`, `set_pricing(rows)` — edit per-1M rates
- `get_setting(key)`, `set_setting(key, value)` — generic key/value (custom log path, etc.)

All Rust types carry both `tokens_total` and `cost_usd`. **Tokens are the primary metric**; cost is secondary and muted in the UI. Recommendations carry both `estimated_savings_tokens` and `estimated_savings_usd`.

## Recommendation rules
Severity-prioritized, all heuristic, all offline. Dismissals stored in `dismissed_recs` keyed by `rec.key`.

- `opus-overuse` — too much Opus on routine work
- `low-cache-reuse` — cache_read / (cache_read + input) below threshold
- `sprawling-{session_id}` — single session running too long / too many turns
- `context-bloat-{session_id}` — context growing without proportional output
- `bash-overuse-{session_id}` — bash dominating tool calls
- `hand-typeable-commands` — repeated bash commands that should be aliased / scripted
- `env-prefix-spam` — redundant `FOO=bar` env prefixes on every invocation

Health signals (positive checks, rendered as the "What's healthy" grid):
`cache-reuse-good`, `model-mix-good`, `sessions-focused`, `sessions-per-task`, `bash-not-dominant`, `hand-typeable-low`, `no-env-spam`, `project-diversity`, `no-context-bloat`.

## UI conventions (load-bearing — don't violate)
- **Tokens primary, cost secondary**: tokens use `text-ink`, costs use `text-muted`.
- **All numbers** go through formatters from `lib/format.ts` (Intl.NumberFormat). No raw `toFixed` in views.
- **Numeric columns**: `shrink-0` + parent `gap-3`. Header widths must match row widths exactly.
- **Truncated text**: `min-w-0 flex-1 truncate` with `title={fullValue}` for hover.
- **Project paths** display via `shortProject()` → `…/CloudDocs/Accounting` (last 2 segments).
- **Model names**: strip `claude-` prefix in tight columns.
- **Light + dark theme.** Toggle persisted in `localStorage["cc.theme"]`, managed by `theme` in [src/lib/store.svelte.ts](src/lib/store.svelte.ts); light mode applied via `:root.light` in [src/app.css](src/app.css). Color tokens in `tailwind.config.js`: `bg`, `panel`, `panel2`, `border`, `ink`, `muted`, `accent`, `ok`, `warn`, `err`.
- **Card pattern**: `<div class="card"><div class="card-title">…</div>…</div>`.

## Performance notes
- Views are lazy-mounted on first visit then kept alive via `class:hidden` (not `{#if}`), so tab switches don't re-run SQLite aggregations.
- Initial refresh on launch ingests only new bytes per file (byte-offset checkpoint).
- For ~35k messages, initial mount of a view takes < 1s.

## Design decisions (locked in)
- **Tauri** (not Electron / SwiftUI / Python web server).
- **Five tabs**: Dashboard (entry), Explorer, Commands, Insights, Settings.
- **Launch-only.** No background agent, no notifications, no menu-bar mode.
- **Heuristic-only recommendations** — no API calls, fully offline.
- **No quota throttling.** Block usage is informational only.
- **No telemetry, no sync, no export.** Local cache only.

## When editing
- **Adding an aggregation**: function in `analyze.rs` → `#[tauri::command]` in `commands.rs` → TS type + wrapper in `ipc.ts` → consume in a view.
- **Editing a recommendation rule**: in `analyze::recommendations`. Always set both `estimated_savings_tokens` and `estimated_savings_usd` (0 is fine if not applicable). Keep `key` stable so dismissals survive.
- **Editing pricing defaults**: `src-tauri/src/pricing.rs`. Runtime overrides live in the `settings` table, edited from the Settings tab.
- **Schema changes**: bump in `db.rs::init_schema`, add an idempotent `CREATE TABLE IF NOT EXISTS` / `ALTER TABLE`. There's no migration framework — keep it idempotent.

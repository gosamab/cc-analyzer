# cc-analyzer — Design

Local-first desktop app for analyzing Claude Code usage. Reads `~/.claude/projects/**/*.jsonl`, surfaces cost/utilization breakdowns, and emits actionable recommendations. Fully offline.

## Principles
1. Zero config — finds logs automatically.
2. Offline & private — no network.
3. Truth over polish — show the real numbers.
4. Action-oriented — every insight ends in a concrete next step.

## Stack
- Tauri 2 (Rust backend, web UI)
- Svelte 5 + TypeScript + Tailwind + uPlot
- SQLite cache at `~/Library/Application Support/cc-analyzer/cache.db`
- Incremental JSONL ingestion (byte-offset checkpoint per file)

## Information architecture
```
Dashboard  → at-a-glance KPIs and bars
Explorer   → sessions list, filters, session detail
Insights   → prioritized recommendations
Settings   → pricing, modes, notifications
```

## Modes
- **Launch-only** (default) — parse on open.
- **Menu bar agent** (M5) — file-watch, advisory notifications.

## Data pipeline
```
~/.claude/projects/*/*.jsonl
   └─ parser.rs (incremental, byte-offset)
       └─ SQLite (messages, file_offsets, dismissed_recs, settings)
           └─ analyze.rs (aggregations, recs, utilization)
               └─ commands.rs (Tauri IPC)
                   └─ Svelte UI
```

## Recommendations catalog (M3)
| Key | Severity | Trigger |
|---|---|---|
| opus-overuse | HIGH | Opus > 70% of spend |
| low-cache-reuse | MED | cache_r/(cache_r+cache_w) < 50% |
| sprawling-session | MED | session > 500 turns |
| repeated-rereads | MED | same file read > 5× |
| bash-overuse | LOW | Bash > 2× Read AND > 100 calls |
| git-status-spam | LOW | `git status` > 30% of git calls |
| context-bloat | HIGH | avg ctx > 300k AND rising |

Each rec carries: `key`, `severity`, `title`, `body`, `evidence` JSON, `estimated_savings_usd_per_month`. Dismissible via `dismissed_recs` table.

## Notifications (M5)
Conservative defaults:
- Context bloat (>300k avg)
- 5-hour block >85%

Rules (opt-in):
- Topic switch detected
- Idle resume on long session
- Cost milestones ($50/$100/$250)
- Repeated re-reads

Guardrails: rate limit (3/h global, 1/h per session), quiet hours, 30-min honeymoon, snooze per-session, per-rule kill switch.

## Milestones
- **M1** Core: parser, schema, Dashboard.  ✅ (skeleton)
- **M2** Explorer: filters, session detail.  ✅ (skeleton)
- **M3** Insights: full rec catalog + patterns.
- **M4** Settings: pricing overrides, plan/quota.
- **M5** Background agent: notify-rs, triggers, notifications.

## Open questions
- Multi-machine sync (iCloud symlink vs strict-local)?
- CSV export per view?
- Timezone display (UTC vs local) — default local.
- Treat resumed sessions (same id, new file) as one chat?

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::db::Db;

#[derive(Serialize)]
pub struct Summary {
    pub total_cost_usd: f64,
    pub msgs: i64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub by_model: Vec<ModelBucket>,
    pub by_project: Vec<ProjectBucket>,
}

#[derive(Serialize)]
pub struct ModelBucket {
    pub model: String,
    pub tokens_total: i64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub cost_usd: f64,
    pub msgs: i64,
}

#[derive(Serialize)]
pub struct ProjectBucket {
    pub project: String,
    pub tokens_total: i64,
    pub cost_usd: f64,
    pub msgs: i64,
    pub sessions: i64,
}

pub fn summary(db: &Db, since: Option<&str>, until: Option<&str>, project: Option<&str>) -> Result<Summary> {
    let (where_sql, args) = build_where(since, until, project);
    let q = format!(
        "SELECT COALESCE(SUM(cost_usd),0), COUNT(*),
                COALESCE(SUM(input_tok),0), COALESCE(SUM(output_tok),0),
                COALESCE(SUM(cache_w_tok),0), COALESCE(SUM(cache_r_tok),0)
         FROM messages {where_sql}"
    );
    let mut stmt = db.conn.prepare(&q)?;
    let row: (f64, i64, i64, i64, i64, i64) =
        stmt.query_row(rusqlite::params_from_iter(args.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
        })?;

    let by_model = grouped_models(db, since, until, project)?;
    let by_project = grouped_projects(db, since, until)?;

    Ok(Summary {
        total_cost_usd: row.0,
        msgs: row.1,
        input_tok: row.2,
        output_tok: row.3,
        cache_w_tok: row.4,
        cache_r_tok: row.5,
        by_model,
        by_project,
    })
}

fn build_where(
    since: Option<&str>,
    until: Option<&str>,
    project: Option<&str>,
) -> (String, Vec<String>) {
    let mut clauses = Vec::new();
    let mut args = Vec::new();
    if let Some(s) = since {
        clauses.push("ts >= ?".to_string());
        args.push(s.to_string());
    }
    if let Some(u) = until {
        clauses.push("ts < ?".to_string());
        args.push(u.to_string());
    }
    if let Some(p) = project {
        clauses.push("project = ?".to_string());
        args.push(p.to_string());
    }
    let s = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    (s, args)
}

fn grouped_models(
    db: &Db,
    since: Option<&str>,
    until: Option<&str>,
    project: Option<&str>,
) -> Result<Vec<ModelBucket>> {
    let (where_sql, args) = build_where(since, until, project);
    let q = format!(
        "SELECT model, SUM(input_tok), SUM(output_tok), SUM(cache_w_tok), SUM(cache_r_tok),
                SUM(cost_usd), COUNT(*)
         FROM messages {where_sql}
         GROUP BY model
         ORDER BY 2+3+4+5 DESC"
    );
    let mut stmt = db.conn.prepare(&q)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let inp: i64 = r.get(1)?;
        let out: i64 = r.get(2)?;
        let cw: i64 = r.get(3)?;
        let cr: i64 = r.get(4)?;
        Ok(ModelBucket {
            model: r.get(0)?,
            tokens_total: inp + out + cw + cr,
            input_tok: inp,
            output_tok: out,
            cache_w_tok: cw,
            cache_r_tok: cr,
            cost_usd: r.get(5)?,
            msgs: r.get(6)?,
        })
    })?;
    let mut v = Vec::new();
    for r in rows {
        v.push(r?);
    }
    Ok(v)
}

fn grouped_projects(
    db: &Db,
    since: Option<&str>,
    until: Option<&str>,
) -> Result<Vec<ProjectBucket>> {
    let (where_sql, args) = build_where(since, until, None);
    let q = format!(
        "SELECT project,
                SUM(input_tok+output_tok+cache_w_tok+cache_r_tok) AS toks,
                SUM(cost_usd), COUNT(*), COUNT(DISTINCT session_id)
         FROM messages {where_sql}
         GROUP BY project
         ORDER BY toks DESC"
    );
    let mut stmt = db.conn.prepare(&q)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        Ok(ProjectBucket {
            project: r.get(0)?,
            tokens_total: r.get(1)?,
            cost_usd: r.get(2)?,
            msgs: r.get(3)?,
            sessions: r.get(4)?,
        })
    })?;
    let mut v = Vec::new();
    for r in rows {
        v.push(r?);
    }
    Ok(v)
}

#[derive(Serialize)]
pub struct DayRow {
    pub day: String,
    pub tokens_total: i64,
    pub cost_usd: f64,
    pub msgs: i64,
    pub sessions: Vec<DaySession>,
}

#[derive(Serialize)]
pub struct DaySession {
    pub session_id: String,
    pub project: String,
    pub tokens_total: i64,
    pub cost_usd: f64,
    pub msgs: i64,
}

pub fn daily_breakdown(db: &Db, since: &str, until: &str) -> Result<Vec<DayRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT substr(ts,1,10) AS day, session_id, project,
                SUM(input_tok+output_tok+cache_w_tok+cache_r_tok) AS toks,
                SUM(cost_usd), COUNT(*)
         FROM messages
         WHERE ts >= ?1 AND ts < ?2
         GROUP BY day, session_id, project
         ORDER BY day ASC, toks DESC",
    )?;
    let mut days: Vec<DayRow> = Vec::new();
    let rows = stmt.query_map(params![since, until], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, f64>(4)?,
            r.get::<_, i64>(5)?,
        ))
    })?;
    for r in rows {
        let (day, sid, proj, toks, cost, msgs) = r?;
        if days.last().map(|d| d.day.as_str()) != Some(day.as_str()) {
            days.push(DayRow {
                day: day.clone(),
                tokens_total: 0,
                cost_usd: 0.0,
                msgs: 0,
                sessions: Vec::new(),
            });
        }
        let d = days.last_mut().unwrap();
        d.tokens_total += toks;
        d.cost_usd += cost;
        d.msgs += msgs;
        d.sessions.push(DaySession {
            session_id: sid,
            project: proj,
            tokens_total: toks,
            cost_usd: cost,
            msgs,
        });
    }
    Ok(days)
}

#[derive(Serialize)]
pub struct SessionRow {
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub msgs: i64,
    pub tokens_total: i64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub cost_usd: f64,
    pub start_ts: String,
    pub end_ts: String,
    pub title: Option<String>,
}

pub fn sessions(db: &Db, since: Option<&str>, until: Option<&str>) -> Result<Vec<SessionRow>> {
    let (where_sql, args) = build_where(since, until, None);
    let q = format!(
        "SELECT m.session_id, m.project, MAX(m.model), COUNT(*),
                SUM(m.input_tok), SUM(m.output_tok), SUM(m.cache_w_tok), SUM(m.cache_r_tok),
                SUM(m.cost_usd), MIN(m.ts), MAX(m.ts), t.title
         FROM messages m
         LEFT JOIN session_titles t ON t.session_id = m.session_id
         {where_sql}
         GROUP BY m.session_id, m.project
         ORDER BY SUM(m.input_tok+m.output_tok+m.cache_w_tok+m.cache_r_tok) DESC
         LIMIT 500"
    );
    // build_where uses unqualified column names — they still match the m alias.
    let mut stmt = db.conn.prepare(&q)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let inp: i64 = r.get(4)?;
        let out: i64 = r.get(5)?;
        let cw: i64 = r.get(6)?;
        let cr: i64 = r.get(7)?;
        Ok(SessionRow {
            session_id: r.get(0)?,
            project: r.get(1)?,
            model: r.get(2)?,
            msgs: r.get(3)?,
            tokens_total: inp + out + cw + cr,
            input_tok: inp,
            output_tok: out,
            cache_w_tok: cw,
            cache_r_tok: cr,
            cost_usd: r.get(8)?,
            start_ts: r.get(9)?,
            end_ts: r.get(10)?,
            title: r.get(11)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct Utilization {
    pub turns: i64,
    pub span_min: f64,
    pub active_min: f64,
    pub utilization_pct: f64,
    pub turns_per_active_hour: f64,
    pub cost_per_active_hour: f64,
    pub avg_context: f64,
    pub avg_output: f64,
    pub output_input_ratio: f64,
    pub blocks: Vec<Block>,
    pub hourly: Vec<HourBucket>,
}

#[derive(Serialize)]
pub struct Block {
    pub start: String,
    pub end: String,
    pub minutes: f64,
    pub turns: i64,
    pub cost_usd: f64,
    pub top_project: String,
}

#[derive(Serialize)]
pub struct HourBucket {
    pub hour: String,
    pub turns: i64,
}

pub fn utilization(db: &Db, day: &str) -> Result<Utilization> {
    let start = format!("{day}T00:00:00");
    let end = format!("{day}T23:59:59");
    let mut stmt = db.conn.prepare(
        "SELECT ts, project, cost_usd,
                (input_tok+cache_w_tok+cache_r_tok) AS ctx, output_tok
         FROM messages
         WHERE ts >= ?1 AND ts <= ?2
         ORDER BY ts ASC",
    )?;
    let mut rows = stmt.query(params![start, end])?;
    let mut data: Vec<(String, String, f64, i64, i64)> = Vec::new();
    while let Some(r) = rows.next()? {
        data.push((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?));
    }

    let empty = Utilization {
        turns: 0,
        span_min: 0.0,
        active_min: 0.0,
        utilization_pct: 0.0,
        turns_per_active_hour: 0.0,
        cost_per_active_hour: 0.0,
        avg_context: 0.0,
        avg_output: 0.0,
        output_input_ratio: 0.0,
        blocks: vec![],
        hourly: vec![],
    };
    if data.is_empty() {
        return Ok(empty);
    }

    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp() as f64);
    let total_cost: f64 = data.iter().map(|r| r.2).sum();
    let total_ctx: i64 = data.iter().map(|r| r.3).sum();
    let total_out: i64 = data.iter().map(|r| r.4).sum();

    let first_ts = parse(&data[0].0).unwrap_or(0.0);
    let last_ts = parse(&data[data.len() - 1].0).unwrap_or(0.0);
    let span_min = (last_ts - first_ts).max(0.0) / 60.0;

    let mut blocks: Vec<Block> = Vec::new();
    let mut cur: Vec<&(String, String, f64, i64, i64)> = vec![&data[0]];
    let gap = 10.0 * 60.0;
    for r in data.iter().skip(1) {
        let prev = parse(&cur.last().unwrap().0).unwrap_or(0.0);
        let now = parse(&r.0).unwrap_or(prev);
        if now - prev > gap {
            blocks.push(make_block(&cur));
            cur.clear();
        }
        cur.push(r);
    }
    blocks.push(make_block(&cur));

    let active_min: f64 = blocks.iter().map(|b| b.minutes).sum();
    let mut hourly_map: HashMap<String, i64> = HashMap::new();
    for r in &data {
        let h = r.0.get(11..13).unwrap_or("00").to_string();
        *hourly_map.entry(h).or_default() += 1;
    }
    let mut hourly: Vec<HourBucket> = hourly_map
        .into_iter()
        .map(|(h, t)| HourBucket { hour: h, turns: t })
        .collect();
    hourly.sort_by(|a, b| a.hour.cmp(&b.hour));

    let n = data.len() as f64;
    Ok(Utilization {
        turns: n as i64,
        span_min,
        active_min,
        utilization_pct: if span_min > 0.0 { active_min / span_min * 100.0 } else { 0.0 },
        turns_per_active_hour: if active_min > 0.0 { n / (active_min / 60.0) } else { 0.0 },
        cost_per_active_hour: if active_min > 0.0 { total_cost / (active_min / 60.0) } else { 0.0 },
        avg_context: total_ctx as f64 / n,
        avg_output: total_out as f64 / n,
        output_input_ratio: if total_ctx > 0 { total_out as f64 / total_ctx as f64 } else { 0.0 },
        blocks,
        hourly,
    })
}

fn make_block(rows: &[&(String, String, f64, i64, i64)]) -> Block {
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp() as f64);
    let start = rows[0].0.clone();
    let end = rows[rows.len() - 1].0.clone();
    let minutes = (parse(&end).unwrap_or(0.0) - parse(&start).unwrap_or(0.0)).max(0.0) / 60.0;
    let cost: f64 = rows.iter().map(|r| r.2).sum();
    let mut proj_count: HashMap<String, i64> = HashMap::new();
    for r in rows {
        *proj_count.entry(r.1.clone()).or_default() += 1;
    }
    let top_project = proj_count
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k)
        .unwrap_or_default();
    Block { start, end, minutes, turns: rows.len() as i64, cost_usd: cost, top_project }
}

#[derive(Serialize)]
pub struct Recommendation {
    pub key: String,
    pub severity: String, // "HIGH" | "MED" | "LOW"
    pub title: String,
    pub body: String,
    pub action: String,
    pub action_session_id: Option<String>,
    pub action_project: Option<String>,
    pub evidence: serde_json::Value,
    pub estimated_savings_tokens: i64,
    pub estimated_savings_usd: f64,
}

#[derive(Serialize)]
pub struct HealthSignal {
    pub key: String,
    pub title: String,
    pub detail: String,
}

pub fn recommendations(db: &Db, since: &str, until: &str) -> Result<Vec<Recommendation>> {
    let mut recs = Vec::new();
    let s = summary(db, Some(since), Some(until), None)?;

    // Rule 1 — Opus over-use
    let opus_tokens: i64 = s
        .by_model
        .iter()
        .filter(|m| m.model.contains("opus"))
        .map(|m| m.tokens_total)
        .sum();
    let opus_cost: f64 = s
        .by_model
        .iter()
        .filter(|m| m.model.contains("opus"))
        .map(|m| m.cost_usd)
        .sum();
    let total_tokens: i64 = s.input_tok + s.output_tok + s.cache_w_tok + s.cache_r_tok;
    if total_tokens > 0 && (opus_tokens as f64 / total_tokens as f64) > 0.7 {
        let top_opus_project = top_opus_project(db, since, until)?;
        let action = match &top_opus_project {
            Some(p) => format!(
                "Switch routine work in {} to Sonnet/Haiku (small edits, lookups, status checks). Reserve Opus for hard reasoning.",
                shorten_project(p)
            ),
            None => "Reserve Opus for hard reasoning; route routine edits and lookups to Sonnet/Haiku.".to_string(),
        };
        recs.push(Recommendation {
            key: "opus-overuse".into(),
            severity: "HIGH".into(),
            title: "Opus dominates token usage".into(),
            body: format!(
                "Opus = {:.0}% of tokens. Routine work (small edits, lookups) on Sonnet/Haiku uses the same tokens at 5–10× lower cost.",
                opus_tokens as f64 / total_tokens as f64 * 100.0
            ),
            action,
            action_session_id: None,
            action_project: top_opus_project,
            evidence: serde_json::json!({
                "opus_tokens": opus_tokens,
                "total_tokens": total_tokens,
                "opus_cost_usd": opus_cost,
            }),
            estimated_savings_tokens: 0,
            estimated_savings_usd: opus_cost * 0.4,
        });
    }

    // Rule 2 — Low cache reuse
    let cw = s.cache_w_tok;
    let cr = s.cache_r_tok;
    if cw + cr > 0 {
        let reuse = cr as f64 / (cw + cr) as f64;
        if reuse < 0.5 {
            let wasted_tokens = (cw as f64 * 0.3) as i64;
            recs.push(Recommendation {
                key: "low-cache-reuse".into(),
                severity: "MED".into(),
                title: "Low prompt-cache reuse".into(),
                body: format!(
                    "Reuse is {:.0}%. You're re-paying cache-write tokens that should be hits. Avoid >5min idle gaps and put stable context (CLAUDE.md) early.",
                    reuse * 100.0
                ),
                action: "Keep sessions warm: don't idle >5 min between turns, and pin stable context (CLAUDE.md, large file reads) early in the conversation so it stays cache-hit.".into(),
                action_session_id: None,
                action_project: None,
                evidence: serde_json::json!({
                    "reuse_pct": reuse * 100.0,
                    "cache_w_tokens": cw,
                    "cache_r_tokens": cr,
                }),
                estimated_savings_tokens: wasted_tokens,
                estimated_savings_usd: wasted_tokens as f64 / 1_000_000.0 * 18.75,
            });
        }
    }

    // Rule 3 — sprawling sessions
    let sess = sessions(db, Some(since), Some(until))?;
    if let Some(big) = sess.iter().find(|s| s.msgs > 500) {
        let bloat_tokens = (big.tokens_total as f64 * 0.2) as i64;
        let short_id = &big.session_id[..8];
        recs.push(Recommendation {
            key: format!("sprawling-{}", big.session_id),
            severity: "MED".into(),
            title: "Sprawling session detected".into(),
            body: format!(
                "Session {} accumulated {} turns and {:.1}M tokens. Each turn replays the full context — /clear between tasks would cut per-turn tokens drastically.",
                short_id,
                big.msgs,
                big.tokens_total as f64 / 1_000_000.0,
            ),
            action: format!(
                "Run /clear in session {} before your next task. Start a fresh session per logical task in {}.",
                short_id,
                shorten_project(&big.project),
            ),
            action_session_id: Some(big.session_id.clone()),
            action_project: Some(big.project.clone()),
            evidence: serde_json::json!({
                "session_id": big.session_id,
                "project": big.project,
                "msgs": big.msgs,
                "tokens_total": big.tokens_total,
                "cost_usd": big.cost_usd,
            }),
            estimated_savings_tokens: bloat_tokens,
            estimated_savings_usd: big.cost_usd * 0.2,
        });
    }

    // Rule 4 — Context bloat (per-turn tokens climb across a session)
    for row in context_bloat_candidates(db, since, until)?.into_iter().take(3) {
        let short_id = &row.session_id[..8];
        let ratio = row.late_avg / row.early_avg.max(1.0);
        let severity = if ratio >= 4.0 { "HIGH" } else { "MED" };
        // Each late turn costs (late_avg - early_avg) extra tokens. Approximate
        // savings as: half the session's late turns × that delta.
        let extra_per_turn = (row.late_avg - row.early_avg).max(0.0);
        let bloated_turns = (row.turns as f64 * 0.5) as i64;
        let saved_tokens = (extra_per_turn * bloated_turns as f64) as i64;
        recs.push(Recommendation {
            key: format!("context-bloat-{}", row.session_id),
            severity: severity.into(),
            title: "Context bloat in long session".into(),
            body: format!(
                "Session {} averages {} tokens/turn at the end vs {} at the start ({:.1}× growth over {} turns). Each later turn replays an inflated context.",
                short_id,
                fmt_tok(row.late_avg as i64),
                fmt_tok(row.early_avg as i64),
                ratio,
                row.turns,
            ),
            action: format!(
                "Run /clear in session {} between subtasks — your last {} turns each carry ~{} extra tokens vs the early ones.",
                short_id,
                bloated_turns,
                fmt_tok(extra_per_turn as i64),
            ),
            action_session_id: Some(row.session_id.clone()),
            action_project: Some(row.project.clone()),
            evidence: serde_json::json!({
                "session_id": row.session_id,
                "project": row.project,
                "turns": row.turns,
                "early_avg_tokens": row.early_avg as i64,
                "late_avg_tokens": row.late_avg as i64,
                "growth_ratio": ratio,
            }),
            estimated_savings_tokens: saved_tokens,
            estimated_savings_usd: saved_tokens as f64 / 1_000_000.0 * 1.50, // cache-read price as floor
        });
    }

    // Rule 5 — Hand-typeable commands (git/install/run-scripts that you could
    // just run yourself instead of paying a turn of context replay).
    if let Some(rec) = hand_typeable_recommendation(db, since, until)? {
        recs.push(rec);
    }

    // Rule 5b — Repeated env-var prefixes (`DOTNET_ROOT=… dotnet …`) that
    // should live in shell init so Claude doesn't restate them every turn.
    if let Some(rec) = env_prefix_recommendation(db, since, until)? {
        recs.push(rec);
    }

    // Rule 6 — Bash overuse (sessions where Bash dominates the tool mix, often
    // git-status-spam or shell-noodling)
    for row in bash_overuse_candidates(db, since, until)?.into_iter().take(3) {
        let short_id = &row.session_id[..8];
        let pct = row.bash_share * 100.0;
        let severity = if row.bash_count >= 200 && row.bash_share >= 0.6 { "HIGH" } else { "MED" };
        let repeat_phrase = match &row.top_repeat {
            Some((cmd, n)) if *n >= 10 => {
                let preview: String = cmd.chars().take(60).collect();
                format!(" `{}` alone ran {} times.", preview, n)
            }
            _ => String::new(),
        };
        recs.push(Recommendation {
            key: format!("bash-overuse-{}", row.session_id),
            severity: severity.into(),
            title: "Bash dominates a session".into(),
            body: format!(
                "Session {} ran Bash {} times — {:.0}% of all tool calls.{} Shell loops re-read state instead of caching it; each call replays the full context.",
                short_id,
                row.bash_count,
                pct,
                repeat_phrase,
            ),
            action: format!(
                "Audit Bash usage in session {}: batch related commands, cache state in memory, or use a Read tool instead of repeated greps/ls.",
                short_id,
            ),
            action_session_id: Some(row.session_id.clone()),
            action_project: Some(row.project.clone()),
            evidence: serde_json::json!({
                "session_id": row.session_id,
                "project": row.project,
                "bash_count": row.bash_count,
                "total_tool_calls": row.total_calls,
                "bash_share": row.bash_share,
                "top_repeat": row.top_repeat,
            }),
            // Conservative: assume half the bash turns were avoidable; charge them
            // at cache-read rate (each turn replays cached context).
            estimated_savings_tokens: 0,
            estimated_savings_usd: 0.0,
        });
    }

    Ok(recs)
}

#[derive(Serialize)]
pub struct CommandRow {
    pub cmd: String,       // most-common raw variant in this group
    pub group_key: String, // the collapsed leading-tokens key used to group
    pub category: String,  // "git" | "install" | "run" | "other"
    pub count: i64,
    pub tokens: i64,
    pub cost_usd: f64,
    pub variants: Vec<CommandVariant>,
}

#[derive(Serialize)]
pub struct CommandVariant {
    pub cmd: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct ToolUsageRow {
    pub tool: String,
    pub count: i64,
    pub tokens: i64,
    pub cost_usd: f64,
    pub turns: i64, // distinct turns that used this tool
}

/// Aggregate tool usage across the range (per tool name). Each tool call inside
/// a turn is charged (turn_tokens / tools_in_turn) so multi-tool turns don't
/// double-count.
pub fn tool_usage(db: &Db, since: &str, until: &str) -> Result<Vec<ToolUsageRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd, tools_json
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    let mut agg: HashMap<String, (i64, f64, i64, i64)> = HashMap::new();
    while let Some(r) = rows.next()? {
        let inp: i64 = r.get(0)?;
        let out: i64 = r.get(1)?;
        let cw: i64 = r.get(2)?;
        let cr: i64 = r.get(3)?;
        let cost: f64 = r.get(4)?;
        let json: String = r.get(5)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        if tools.is_empty() { continue; }
        let n_tools = tools.len() as f64;
        let turn_tokens = (inp + out + cw + cr) as f64;
        let per_call_tokens = (turn_tokens / n_tools) as i64;
        let per_call_cost = cost / n_tools;
        let mut turn_tools: std::collections::HashSet<String> = std::collections::HashSet::new();
        for t in tools {
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?").to_string();
            let entry = agg.entry(name.clone()).or_insert((0, 0.0, 0, 0));
            entry.0 += 1;
            entry.1 += per_call_cost;
            entry.2 += per_call_tokens;
            turn_tools.insert(name);
        }
        for name in turn_tools {
            agg.entry(name).or_insert((0, 0.0, 0, 0)).3 += 1;
        }
    }
    let mut out: Vec<ToolUsageRow> = agg
        .into_iter()
        .map(|(tool, (count, cost, tokens, turns))| ToolUsageRow {
            tool,
            count,
            tokens,
            cost_usd: cost,
            turns,
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    Ok(out)
}

/// Top Bash commands in the range with attributed token + cost cost.
/// Each tool call inside a turn is charged (turn_tokens / tools_in_turn) — so
/// a multi-tool turn splits its replay cost across its calls. Commands are
/// grouped by their leading-tokens key; each row carries the raw variants
/// that fell into the group so the UI can expand to show them.
pub fn top_commands(db: &Db, since: &str, until: &str, limit: i64) -> Result<Vec<CommandRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd, tools_json
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    struct Agg {
        tokens: i64,
        cost: f64,
        count: i64,
        category: &'static str,
        variants: HashMap<String, i64>,
    }
    let mut agg: HashMap<String, Agg> = HashMap::new();
    while let Some(r) = rows.next()? {
        let inp: i64 = r.get(0)?;
        let out: i64 = r.get(1)?;
        let cw: i64 = r.get(2)?;
        let cr: i64 = r.get(3)?;
        let cost: f64 = r.get(4)?;
        let json: String = r.get(5)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        if tools.is_empty() { continue; }
        let n_tools = tools.len() as f64;
        let turn_tokens = (inp + out + cw + cr) as f64;
        let per_call_tokens = (turn_tokens / n_tools) as i64;
        let per_call_cost = cost / n_tools;
        for t in tools {
            if t.get("name").and_then(|v| v.as_str()) != Some("Bash") { continue; }
            let Some(cmd) = t.get("cmd").and_then(|v| v.as_str()) else { continue };
            let normalized = normalize_cmd(cmd);
            let cat = command_category(&normalized);
            let key = cmd_display_key(&normalized);
            let entry = agg.entry(key).or_insert_with(|| Agg {
                tokens: 0,
                cost: 0.0,
                count: 0,
                category: cat,
                variants: HashMap::new(),
            });
            entry.tokens += per_call_tokens;
            entry.cost += per_call_cost;
            entry.count += 1;
            // Truncate the raw variant for storage — most commands fit in <120 chars
            // and full text is already in tools_json if anyone needs it.
            let raw: String = cmd.chars().take(200).collect();
            *entry.variants.entry(raw).or_default() += 1;
        }
    }
    let mut out: Vec<CommandRow> = agg
        .into_iter()
        .map(|(group_key, a)| {
            let mut variants: Vec<CommandVariant> = a
                .variants
                .into_iter()
                .map(|(cmd, count)| CommandVariant { cmd, count })
                .collect();
            // Sort by count desc; tie-break by shorter cmd so when many heredoc
            // variants tie at count=1 we pick the cleanest one as the label.
            variants.sort_by(|x, y| {
                y.count.cmp(&x.count).then_with(|| x.cmd.len().cmp(&y.cmd.len()))
            });
            let top_cmd = variants
                .first()
                .map(|v| oneline(&v.cmd))
                .unwrap_or_else(|| group_key.clone());
            // Cap variant list so the IPC payload stays small for cd/grep-style sprawl.
            variants.truncate(20);
            CommandRow {
                cmd: top_cmd,
                group_key,
                category: a.category.to_string(),
                count: a.count,
                tokens: a.tokens,
                cost_usd: a.cost,
                variants,
            }
        })
        .collect();
    out.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    out.truncate(limit.max(1) as usize);
    Ok(out)
}

/// Broader categorization used by the Commands table — gives every row a
/// meaningful bucket. The "hand-typeable" set (git/install/run) overlaps
/// with this; new buckets (search/fs/inspect/script/text/net) are purely
/// for display, not flagged by the recommendation.
fn command_category(cmd: &str) -> &'static str {
    if let Some(c) = hand_typeable_category(cmd) {
        return c;
    }
    let head = cmd.split_whitespace().next().unwrap_or("");
    // Strip a `./` or absolute-path prefix to compare basename.
    let basename = head.rsplit('/').next().unwrap_or(head);
    match basename {
        // Search/find
        "grep" | "rg" | "ag" | "ack" | "find" | "fd" | "locate" => "search",
        // Filesystem nav/mutation
        "cd" | "pushd" | "popd" | "ls" | "ll" | "la" | "mkdir" | "rmdir" | "cp" | "mv"
        | "rm" | "ln" | "touch" | "chmod" | "chown" | "stat" | "readlink" | "realpath"
        | "pwd" => "fs",
        // Reading file content
        "cat" | "bat" | "head" | "tail" | "less" | "more" | "wc" | "file" | "du" | "df"
        | "tree" | "tac" | "nl" | "hexdump" | "xxd" => "inspect",
        // Interpreters / script runners
        "python" | "python3" | "py" | "node" | "deno" | "bun" | "ruby" | "perl"
        | "bash" | "sh" | "zsh" | "fish" | "tsx" | "ts-node" | "ipython" => "script",
        // Text-processing pipelines
        "sed" | "awk" | "tr" | "sort" | "uniq" | "cut" | "paste" | "jq" | "yq"
        | "fzf" | "xargs" | "tee" | "column" => "text",
        // Network / remote
        "curl" | "wget" | "ssh" | "scp" | "rsync" | "ping" | "dig" | "nslookup"
        | "nc" | "telnet" | "host" | "traceroute" => "net",
        _ => "other",
    }
}

/// Categorize a Bash command into one of the "hand-typeable" buckets, or None
/// if the command is something Claude probably should be running (file ops,
/// piped state-extraction, etc.).
fn hand_typeable_category(cmd: &str) -> Option<&'static str> {
    let c = cmd.trim_start();
    let head: String = c.chars().take(40).collect();
    let h = head.as_str();
    // Strip a leading subshell or env-prefix if present; cheap heuristic.
    if h.starts_with("git ") || h == "git" {
        return Some("git");
    }
    if h.starts_with("npm install") || h.starts_with("npm i ") || h == "npm i"
        || h.starts_with("pnpm install") || h.starts_with("pnpm add ")
        || h.starts_with("pnpm i ") || h == "pnpm i"
        || h.starts_with("yarn install") || h.starts_with("yarn add ") || h == "yarn"
        || h.starts_with("bun install") || h.starts_with("bun add ") || h.starts_with("bun i ")
        || h.starts_with("cargo install ") || h.starts_with("cargo add ")
    {
        return Some("install");
    }
    if h.starts_with("npm run") || h.starts_with("npm test") || h.starts_with("npm start")
        || h.starts_with("pnpm run") || h.starts_with("pnpm dev") || h.starts_with("pnpm build")
        || h.starts_with("pnpm test") || h.starts_with("pnpm start")
        || h.starts_with("pnpm exec ") || h.starts_with("pnpm --filter ")
        || h.starts_with("yarn run") || h.starts_with("yarn dev") || h.starts_with("yarn build")
        || h.starts_with("yarn test") || h.starts_with("yarn start")
        || h.starts_with("bun run") || h.starts_with("bun dev") || h.starts_with("bun test")
        || h.starts_with("cargo run") || h.starts_with("cargo build")
        || h.starts_with("cargo test") || h.starts_with("cargo check")
        || h.starts_with("npx ") || h == "npx"
        || h.starts_with("dotnet ") || h == "dotnet"
        || h.starts_with("flutter ") || h == "flutter"
        || h.starts_with("uvicorn ") || h == "uvicorn"
        || h.starts_with("gunicorn ") || h == "gunicorn"
        || h.starts_with("rails ") || h == "rails"
        || h.starts_with("mix ") || h == "mix"
        || h.starts_with("gradle ") || h == "gradle"
        || h.starts_with("mvn ") || h == "mvn"
        || h.starts_with("./gradlew") || h.starts_with("./mvnw")
        || h.starts_with("make ") || h == "make"
    {
        return Some("run");
    }
    None
}

/// Normalize a raw Bash command down to the "real" command being run, so
/// stats group like-with-like:
///   - `cd /tmp/foo && pnpm dev`            → `pnpm dev`
///   - `DOTNET_ROOT=/opt/... dotnet build`  → `dotnet build`
///   - `pnpm exec svelte-check 2>&1 | tail` → `pnpm exec svelte-check`
///   - `cd /tmp/foo`                        → `cd /tmp/foo` (only segment)
/// Quote/subshell handling is intentionally naive — for stats use the
/// occasional `echo "a && b"` misclassification doesn't matter.
fn normalize_cmd(raw: &str) -> String {
    // First split on `;`, then on `&&` / `||` within each segment.
    let segments: Vec<String> = raw
        .split(';')
        .flat_map(split_logical)
        .map(|s| trim_pipe_redirect(s.trim()).trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Walk segments; prefer the last non-trivial one. Fall back to the
    // first trivial segment so single-`cd` calls still get classified.
    let mut chosen: Option<String> = None;
    for seg in &segments {
        let stripped = strip_env_prefix(seg).trim().to_string();
        if stripped.is_empty() { continue; }
        if is_trivial(&stripped) {
            if chosen.is_none() {
                chosen = Some(stripped);
            }
        } else {
            chosen = Some(stripped);
        }
    }
    chosen.unwrap_or_else(|| raw.trim().to_string())
}

fn split_logical(s: &str) -> Vec<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        let pair = &bytes[i..i + 2];
        if pair == b"&&" || pair == b"||" {
            out.push(s[start..i].to_string());
            i += 2;
            start = i;
        } else {
            i += 1;
        }
    }
    out.push(s[start..].to_string());
    out
}

fn trim_pipe_redirect(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        // Skip `||` — that's a logical OR (split_logical handles it, but be safe).
        if c == b'|' && i + 1 < bytes.len() && bytes[i + 1] == b'|' {
            i += 2;
            continue;
        }
        // `<<` (heredoc) is part of the invocation, not a redirect — keep `<<`
        // so `python3 << 'EOF'\n…` collapses to `python3 <<` rather than `python3`.
        if c == b'<' && i + 1 < bytes.len() && bytes[i + 1] == b'<' {
            return &s[..i + 2];
        }
        // `&>` (bash redirect-all) — treat as redirect, cut here.
        if c == b'|' || c == b'<' || c == b'>' {
            return &s[..i];
        }
        i += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_strips_cd_prefix() {
        assert_eq!(normalize_cmd("cd /tmp/foo && pnpm dev"), "pnpm dev");
        assert_eq!(
            normalize_cmd("cd \"/Users/oazab/Library/Mobile Documents/com~apple~CloudDocs/Private/Accounting\" && python3 -c \"import sys\""),
            "python3 -c \"import sys\""
        );
    }

    #[test]
    fn normalize_keeps_heredoc_marker() {
        let raw = "cd \"/tmp\" && python3 << 'EOF'\nimport sys\nfor i in range(10):\n    print(i)\nEOF";
        let n = normalize_cmd(raw);
        assert!(n.starts_with("python3 <<"), "got: {n}");
        // python3 isn't in the keep_subcommand list — collapses to bare `python3`.
        assert_eq!(cmd_display_key(&n), "python3");
    }

    #[test]
    fn normalize_strips_env_prefix() {
        assert_eq!(
            normalize_cmd("DOTNET_ROOT=/opt/homebrew/opt/dotnet@8/libexec dotnet build"),
            "dotnet build"
        );
        assert_eq!(
            normalize_cmd("FOO=bar BAZ=qux cargo run -p client"),
            "cargo run -p client"
        );
    }

    #[test]
    fn normalize_strips_pipe_tail() {
        assert_eq!(
            normalize_cmd("pnpm exec svelte-check --output human 2>&1 | tail -10"),
            "pnpm exec svelte-check --output human 2"
        );
        // Display key for a runner: program + first non-flag/non-pkg arg.
        assert_eq!(
            cmd_display_key(&normalize_cmd("pnpm exec svelte-check --output human 2>&1 | tail -10")),
            "pnpm exec"
        );
    }

    #[test]
    fn display_key_collapses_plain_programs() {
        assert_eq!(cmd_display_key("python3 -c \"import sys\""), "python3");
        assert_eq!(cmd_display_key("python3 script.py --foo"), "python3");
        assert_eq!(cmd_display_key("cd /tmp/foo"), "cd");
        assert_eq!(cmd_display_key("grep -rn pattern src/"), "grep");
        assert_eq!(cmd_display_key("ls -la"), "ls");
    }

    #[test]
    fn display_key_keeps_subcommand_for_runners() {
        assert_eq!(cmd_display_key("git status -sb"), "git status");
        assert_eq!(cmd_display_key("git status --short"), "git status");
        assert_eq!(cmd_display_key("git log --oneline -5"), "git log");
        assert_eq!(cmd_display_key("pnpm dev"), "pnpm dev");
        assert_eq!(cmd_display_key("pnpm run dev"), "pnpm run");
        assert_eq!(cmd_display_key("pnpm --filter @masar/web build"), "pnpm build");
        assert_eq!(cmd_display_key("cargo run -p client"), "cargo run");
        assert_eq!(cmd_display_key("docker ps -a"), "docker ps");
    }

    #[test]
    fn normalize_handles_lone_cd() {
        // No non-trivial segment — keep the cd so it doesn't disappear entirely.
        assert_eq!(normalize_cmd("cd /tmp/foo"), "cd /tmp/foo");
    }

    #[test]
    fn normalize_handles_chained_trivial() {
        // cd → cd → npm test : pick the npm test as the real command.
        assert_eq!(
            normalize_cmd("cd foo && cd bar && npm test"),
            "npm test"
        );
    }
}

fn strip_env_prefix(s: &str) -> &str {
    let mut rest = s.trim_start();
    loop {
        let Some(space) = rest.find(char::is_whitespace) else { break };
        let tok = &rest[..space];
        if is_env_assignment(tok) {
            rest = rest[space..].trim_start();
        } else {
            break;
        }
    }
    rest
}

fn is_env_assignment(tok: &str) -> bool {
    let Some(eq) = tok.find('=') else { return false };
    if eq == 0 { return false; }
    let name = &tok[..eq];
    let mut chars = name.chars();
    let first = match chars.next() { Some(c) => c, None => return false };
    if !(first.is_ascii_uppercase() || first == '_') { return false; }
    chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Collapse runs of whitespace (incl. newlines) to single spaces and trim.
/// Used for row labels so heredoc bodies don't bleed into the display.
fn oneline(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Walk leading `ENV=VAL` tokens, returning each assignment verbatim.
fn extract_env_assignments(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = cmd.trim_start();
    loop {
        let Some(space) = rest.find(char::is_whitespace) else { break };
        let tok = &rest[..space];
        if is_env_assignment(tok) {
            out.push(tok.to_string());
            rest = rest[space..].trim_start();
        } else {
            break;
        }
    }
    out
}

fn is_trivial(seg: &str) -> bool {
    let first = seg.split_whitespace().next().unwrap_or("");
    matches!(
        first,
        "cd" | "export" | "mkdir" | "source" | "." | "unset" | "set" | "true" | "false" | "pushd" | "popd"
    )
}

/// Collapse a command to a stable display key. For most programs the program
/// name alone is the group key (`python3`, `cd`, `grep`, `ls`). For "verb-noun"
/// CLIs (git, package managers, cloud SDKs) we keep the first non-flag arg as
/// the subcommand (`git status`, `pnpm dev`, `cargo run`, `docker ps`).
/// Flags and package selectors (`--filter @foo`, `-p client`) are skipped so
/// `pnpm --filter @foo build` collapses with `pnpm build`.
fn cmd_display_key(cmd: &str) -> String {
    let toks: Vec<&str> = cmd.split_whitespace().collect();
    if toks.is_empty() {
        return String::new();
    }
    let first = toks[0];

    let keep_subcommand = matches!(
        first,
        "git" | "npm" | "pnpm" | "yarn" | "bun" | "npx"
            | "cargo" | "go" | "deno" | "rustup"
            | "dotnet" | "flutter" | "rails" | "mix" | "gradle" | "mvn"
            | "docker" | "podman" | "kubectl" | "helm" | "terraform"
            | "gcloud" | "aws" | "az" | "vercel" | "flyctl" | "heroku"
            | "brew" | "apt" | "apt-get" | "dnf" | "yum" | "pacman"
            | "make" | "just" | "task" | "rake"
            | "systemctl" | "service" | "launchctl"
            | "rbenv" | "pyenv" | "asdf" | "nvm" | "fnm"
            | "pip" | "pip3" | "poetry" | "uv" | "pipx"
    );

    if !keep_subcommand || toks.len() < 2 {
        return first.to_string();
    }

    // Find first arg-like token after the program: skip flags (-x, --foo) and
    // package selectors (@scope/name, paths starting with `/` or `.`).
    let sub = toks.iter().skip(1).find(|t| {
        !t.starts_with('-') && !t.starts_with('@')
    });
    match sub {
        Some(s) => format!("{} {}", first, s),
        None => first.to_string(),
    }
}

fn hand_typeable_recommendation(db: &Db, since: &str, until: &str) -> Result<Option<Recommendation>> {
    let mut stmt = db.conn.prepare(
        "SELECT input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd, tools_json
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    let mut by_cat: HashMap<&'static str, i64> = HashMap::new();
    let mut by_cmd: HashMap<String, (i64, &'static str)> = HashMap::new();
    let mut total = 0i64;
    let mut attributed_tokens: i64 = 0;
    let mut attributed_cost: f64 = 0.0;
    while let Some(r) = rows.next()? {
        let inp: i64 = r.get(0)?;
        let out: i64 = r.get(1)?;
        let cw: i64 = r.get(2)?;
        let cr: i64 = r.get(3)?;
        let cost: f64 = r.get(4)?;
        let json: String = r.get(5)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        if tools.is_empty() { continue; }
        let n_tools = tools.len() as f64;
        let per_call_tokens = ((inp + out + cw + cr) as f64 / n_tools) as i64;
        let per_call_cost = cost / n_tools;
        for t in tools {
            if t.get("name").and_then(|v| v.as_str()) != Some("Bash") {
                continue;
            }
            let Some(cmd) = t.get("cmd").and_then(|v| v.as_str()) else { continue };
            let normalized = normalize_cmd(cmd);
            let Some(cat) = hand_typeable_category(&normalized) else { continue };
            total += 1;
            attributed_tokens += per_call_tokens;
            attributed_cost += per_call_cost;
            *by_cat.entry(cat).or_default() += 1;
            let key = cmd_display_key(&normalized);
            by_cmd.entry(key).or_insert((0, cat)).0 += 1;
        }
    }

    if total < 30 {
        return Ok(None);
    }

    let mut top: Vec<(String, i64, &'static str)> = by_cmd
        .into_iter()
        .map(|(k, (n, c))| (k, n, c))
        .collect();
    top.sort_by(|a, b| b.1.cmp(&a.1));
    top.truncate(5);

    let cat_order = ["git", "run", "install"];
    let cat_str = cat_order
        .iter()
        .filter_map(|c| by_cat.get(c).map(|n| format!("{} {}", n, c)))
        .collect::<Vec<_>>()
        .join(", ");

    let action = format!(
        "Top: {}. Run these in your terminal — each call here replays your full session context.",
        top.iter()
            .map(|(cmd, n, _)| format!("`{}` ({}×)", cmd, n))
            .collect::<Vec<_>>()
            .join(", "),
    );

    let severity = if total >= 200 { "HIGH" } else { "MED" };
    // ~half of these are the kind Claude doesn't actually need the output for —
    // the rest you'd paste back. Conservative 50% recoverable.
    let savings_tokens = (attributed_tokens as f64 * 0.5) as i64;
    let savings_usd = attributed_cost * 0.5;

    Ok(Some(Recommendation {
        key: "hand-typeable-commands".into(),
        severity: severity.into(),
        title: "Commands you could run by hand".into(),
        body: format!(
            "Claude ran {} git/install/run-script commands ({}). Each one costs a full assistant turn — running them in your own terminal skips the context replay.",
            total, cat_str,
        ),
        action,
        action_session_id: None,
        action_project: None,
        evidence: serde_json::json!({
            "total": total,
            "by_category": by_cat,
            "attributed_tokens": attributed_tokens,
            "attributed_cost_usd": attributed_cost,
            "top_commands": top
                .iter()
                .map(|(cmd, n, cat)| serde_json::json!({"cmd": cmd, "count": n, "category": cat}))
                .collect::<Vec<_>>(),
        }),
        estimated_savings_tokens: savings_tokens,
        estimated_savings_usd: savings_usd,
    }))
}

/// Detect env-var prefixes that show up across many Bash calls
/// (e.g. `DOTNET_ROOT=/opt/homebrew/opt/dotnet@8/libexec dotnet …`).
/// These belong in shell init so Claude isn't restating them every turn.
fn env_prefix_recommendation(db: &Db, since: &str, until: &str) -> Result<Option<Recommendation>> {
    let mut stmt = db.conn.prepare(
        "SELECT input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd, tools_json
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    // Per env-var NAME: total count + an example assignment (the most common value).
    let mut by_var: HashMap<String, (i64, HashMap<String, i64>)> = HashMap::new();
    let mut total_with_env: i64 = 0;
    let mut attributed_tokens: i64 = 0;
    let mut attributed_cost: f64 = 0.0;
    while let Some(r) = rows.next()? {
        let inp: i64 = r.get(0)?;
        let out: i64 = r.get(1)?;
        let cw: i64 = r.get(2)?;
        let cr: i64 = r.get(3)?;
        let cost: f64 = r.get(4)?;
        let json: String = r.get(5)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        if tools.is_empty() { continue; }
        let n_tools = tools.len() as f64;
        let per_call_tokens = ((inp + out + cw + cr) as f64 / n_tools) as i64;
        let per_call_cost = cost / n_tools;
        for t in tools {
            if t.get("name").and_then(|v| v.as_str()) != Some("Bash") { continue; }
            let Some(cmd) = t.get("cmd").and_then(|v| v.as_str()) else { continue };
            let assignments = extract_env_assignments(cmd);
            if assignments.is_empty() { continue; }
            total_with_env += 1;
            attributed_tokens += per_call_tokens;
            attributed_cost += per_call_cost;
            for a in assignments {
                let Some(eq) = a.find('=') else { continue };
                let name = a[..eq].to_string();
                let entry = by_var.entry(name).or_insert((0, HashMap::new()));
                entry.0 += 1;
                *entry.1.entry(a.clone()).or_default() += 1;
            }
        }
    }

    // Keep vars that appear >= 10 times — anything less is noise.
    let mut hits: Vec<(String, i64, String)> = by_var
        .into_iter()
        .filter(|(_, (n, _))| *n >= 10)
        .map(|(name, (n, vals))| {
            let example = vals
                .into_iter()
                .max_by_key(|(_, c)| *c)
                .map(|(v, _)| v)
                .unwrap_or_else(|| name.clone());
            (name, n, example)
        })
        .collect();
    if hits.is_empty() {
        return Ok(None);
    }
    hits.sort_by(|a, b| b.1.cmp(&a.1));
    hits.truncate(8);

    let top_n: i64 = hits.iter().map(|(_, n, _)| *n).sum();
    let severity = if top_n >= 100 { "MED" } else { "LOW" };
    let examples = hits
        .iter()
        .take(3)
        .map(|(_, n, ex)| format!("`{}` ({}×)", ex, n))
        .collect::<Vec<_>>()
        .join(", ");
    let export_lines = hits
        .iter()
        .map(|(_, _, ex)| format!("export {}", ex))
        .collect::<Vec<_>>()
        .join("; ");

    Ok(Some(Recommendation {
        key: "env-prefix-spam".into(),
        severity: severity.into(),
        title: "Repeated env-var prefixes on Bash calls".into(),
        body: format!(
            "Claude prefixed {} Bash calls with env-var assignments (top: {}). Each turn restates the prefix and pays for it in context.",
            total_with_env, examples,
        ),
        action: format!(
            "Add to your shell init (~/.zshrc or ~/.bashrc) so Claude can drop the prefix: {}",
            export_lines,
        ),
        action_session_id: None,
        action_project: None,
        evidence: serde_json::json!({
            "total_calls_with_env_prefix": total_with_env,
            "attributed_tokens": attributed_tokens,
            "attributed_cost_usd": attributed_cost,
            "vars": hits
                .iter()
                .map(|(name, n, ex)| serde_json::json!({"name": name, "count": n, "example": ex}))
                .collect::<Vec<_>>(),
        }),
        // Removing the prefix doesn't itself save the full turn — but the
        // whole calling pattern is suspect. Show 10% as a conservative nudge.
        estimated_savings_tokens: (attributed_tokens as f64 * 0.10) as i64,
        estimated_savings_usd: attributed_cost * 0.10,
    }))
}

struct BashRow {
    session_id: String,
    project: String,
    bash_count: i64,
    total_calls: i64,
    bash_share: f64,
    top_repeat: Option<(String, i64)>,
}

fn bash_overuse_candidates(db: &Db, since: &str, until: &str) -> Result<Vec<BashRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT session_id, project, tools_json
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    struct Agg {
        project: String,
        bash: i64,
        total: i64,
        cmds: HashMap<String, i64>,
    }
    let mut per_session: HashMap<String, Agg> = HashMap::new();
    while let Some(r) = rows.next()? {
        let session_id: String = r.get(0)?;
        let project: String = r.get(1)?;
        let json: String = r.get(2)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        if tools.is_empty() { continue; }
        let agg = per_session.entry(session_id).or_insert_with(|| Agg {
            project: project.clone(),
            bash: 0,
            total: 0,
            cmds: HashMap::new(),
        });
        for t in tools {
            agg.total += 1;
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name == "Bash" {
                agg.bash += 1;
                if let Some(cmd) = t.get("cmd").and_then(|v| v.as_str()) {
                    // Group by the leading verb so `git status -sb` and `git status` collapse.
                    let normalized = normalize_cmd(cmd);
                    let key = cmd_display_key(&normalized);
                    if !key.is_empty() {
                        *agg.cmds.entry(key).or_default() += 1;
                    }
                }
            }
        }
    }

    let mut out: Vec<BashRow> = per_session
        .into_iter()
        .filter(|(_, a)| a.bash >= 50 && a.total > 0 && (a.bash as f64 / a.total as f64) >= 0.5)
        .map(|(session_id, a)| {
            let top_repeat = a
                .cmds
                .into_iter()
                .max_by_key(|(_, n)| *n)
                .map(|(k, n)| (k, n));
            BashRow {
                session_id,
                project: a.project,
                bash_count: a.bash,
                total_calls: a.total,
                bash_share: a.bash as f64 / a.total as f64,
                top_repeat,
            }
        })
        .collect();
    out.sort_by(|a, b| b.bash_count.cmp(&a.bash_count));
    Ok(out)
}

struct BloatRow {
    session_id: String,
    project: String,
    turns: i64,
    early_avg: f64,
    late_avg: f64,
}

fn context_bloat_candidates(db: &Db, since: &str, until: &str) -> Result<Vec<BloatRow>> {
    // Use window functions to bucket the first and last quartile of turns per session,
    // compare their average per-turn tokens. Sessions need >= 40 turns to be considered.
    let mut stmt = db.conn.prepare(
        "WITH ranked AS (
             SELECT session_id, project,
                    (input_tok + output_tok + cache_w_tok + cache_r_tok) AS tok,
                    ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY ts) AS rn,
                    COUNT(*) OVER (PARTITION BY session_id) AS cnt
             FROM messages
             WHERE ts >= ?1 AND ts < ?2
         )
         SELECT session_id, project, cnt,
                AVG(CASE WHEN rn <= cnt/4 THEN tok END) AS early_avg,
                AVG(CASE WHEN rn > cnt - cnt/4 THEN tok END) AS late_avg
         FROM ranked
         GROUP BY session_id
         HAVING cnt >= 40
            AND early_avg > 0
            AND late_avg >= 2.0 * early_avg
         ORDER BY late_avg DESC
         LIMIT 10",
    )?;
    let rows = stmt.query_map(params![since, until], |r| {
        Ok(BloatRow {
            session_id: r.get(0)?,
            project: r.get(1)?,
            turns: r.get(2)?,
            early_avg: r.get(3)?,
            late_avg: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

fn fmt_tok(n: i64) -> String {
    if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn top_opus_project(db: &Db, since: &str, until: &str) -> Result<Option<String>> {
    let mut stmt = db.conn.prepare(
        "SELECT project, SUM(input_tok+output_tok+cache_w_tok+cache_r_tok) AS toks
         FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND model LIKE '%opus%'
         GROUP BY project
         ORDER BY toks DESC
         LIMIT 1",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    if let Some(r) = rows.next()? {
        Ok(Some(r.get::<_, String>(0)?))
    } else {
        Ok(None)
    }
}

fn shorten_project(p: &str) -> String {
    let parts: Vec<&str> = p.rsplit('/').take(2).collect();
    if parts.len() == 2 {
        format!("…/{}/{}", parts[1], parts[0])
    } else {
        p.to_string()
    }
}

pub fn health_signals(db: &Db, since: &str, until: &str) -> Result<Vec<HealthSignal>> {
    let mut out = Vec::new();
    let s = summary(db, Some(since), Some(until), None)?;
    let total_tokens: i64 = s.input_tok + s.output_tok + s.cache_w_tok + s.cache_r_tok;

    let cw = s.cache_w_tok;
    let cr = s.cache_r_tok;
    if cw + cr > 0 {
        let reuse = cr as f64 / (cw + cr) as f64;
        if reuse >= 0.7 {
            out.push(HealthSignal {
                key: "cache-reuse-good".into(),
                title: "Excellent prompt-cache reuse".into(),
                detail: format!("{:.0}% of cache tokens are hits — sessions are staying warm.", reuse * 100.0),
            });
        }
    }

    let opus_tokens: i64 = s
        .by_model
        .iter()
        .filter(|m| m.model.contains("opus"))
        .map(|m| m.tokens_total)
        .sum();
    if total_tokens > 0 {
        let opus_share = opus_tokens as f64 / total_tokens as f64;
        if opus_share <= 0.4 && s.by_model.len() > 1 {
            out.push(HealthSignal {
                key: "model-mix-good".into(),
                title: "Balanced model mix".into(),
                detail: format!("Opus is only {:.0}% of tokens — routine work is going to cheaper models.", opus_share * 100.0),
            });
        }
    }

    let sess = sessions(db, Some(since), Some(until))?;
    if !sess.is_empty() {
        let max_msgs = sess.iter().map(|x| x.msgs).max().unwrap_or(0);
        if max_msgs < 200 {
            out.push(HealthSignal {
                key: "sessions-focused".into(),
                title: "Sessions stay focused".into(),
                detail: format!("Biggest session is {} turns — good /clear discipline.", max_msgs),
            });
        }
        let avg_msgs = sess.iter().map(|x| x.msgs).sum::<i64>() as f64 / sess.len() as f64;
        if sess.len() >= 10 && avg_msgs < 80.0 {
            out.push(HealthSignal {
                key: "sessions-per-task".into(),
                title: "One session per task".into(),
                detail: format!("{} sessions in range, average {:.0} turns each.", sess.len(), avg_msgs),
            });
        }
    }

    // Walk tool calls once to derive: bash share, hand-typeable count, env-prefix usage.
    let mut tool_total = 0i64;
    let mut bash_total = 0i64;
    let mut hand_typeable_count = 0i64;
    let mut env_call_count = 0i64;
    let mut env_var_counts: HashMap<String, i64> = HashMap::new();
    let mut stmt = db.conn.prepare(
        "SELECT tools_json FROM messages
         WHERE ts >= ?1 AND ts < ?2 AND tools_json != '[]'",
    )?;
    let mut rows = stmt.query(params![since, until])?;
    while let Some(r) = rows.next()? {
        let json: String = r.get(0)?;
        let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else { continue };
        for t in tools {
            tool_total += 1;
            if t.get("name").and_then(|v| v.as_str()) != Some("Bash") { continue; }
            bash_total += 1;
            let Some(cmd) = t.get("cmd").and_then(|v| v.as_str()) else { continue };
            let assignments = extract_env_assignments(cmd);
            if !assignments.is_empty() {
                env_call_count += 1;
                for a in assignments {
                    if let Some(eq) = a.find('=') {
                        *env_var_counts.entry(a[..eq].to_string()).or_default() += 1;
                    }
                }
            }
            let normalized = normalize_cmd(cmd);
            if hand_typeable_category(&normalized).is_some() {
                hand_typeable_count += 1;
            }
        }
    }

    if tool_total >= 50 {
        let bash_share = bash_total as f64 / tool_total as f64;
        if bash_share < 0.40 {
            out.push(HealthSignal {
                key: "bash-not-dominant".into(),
                title: "Tool mix is edit-heavy".into(),
                detail: format!(
                    "Bash is only {:.0}% of tool calls — most work is reads/edits, not shell.",
                    bash_share * 100.0
                ),
            });
        }
        if hand_typeable_count < 30 {
            out.push(HealthSignal {
                key: "hand-typeable-low".into(),
                title: "Few hand-typeable commands".into(),
                detail: format!(
                    "Only {} git/install/run-script commands ran — Claude isn't burning turns on trivia.",
                    hand_typeable_count
                ),
            });
        }
    }
    let max_var = env_var_counts.values().copied().max().unwrap_or(0);
    if bash_total >= 50 && max_var < 10 {
        out.push(HealthSignal {
            key: "no-env-spam".into(),
            title: "No env-var prefix spam".into(),
            detail: if env_call_count == 0 {
                "No Bash calls carry env-var prefixes.".into()
            } else {
                format!("Env prefixes used sparingly ({} Bash calls, no var repeated ≥10×).", env_call_count)
            },
        });
    }

    if s.by_project.len() >= 3 {
        out.push(HealthSignal {
            key: "project-diversity".into(),
            title: "Multi-project activity".into(),
            detail: format!("{} projects with activity in this range.", s.by_project.len()),
        });
    }

    let bloated = context_bloat_candidates(db, since, until)?;
    if bloated.is_empty() && sess.len() >= 5 {
        out.push(HealthSignal {
            key: "no-context-bloat".into(),
            title: "No context bloat".into(),
            detail: "No session shows runaway per-turn token growth.".into(),
        });
    }

    Ok(out)
}

#[derive(Serialize)]
pub struct SessionDetail {
    pub session_id: String,
    pub project: String,
    pub model: String,
    pub title: Option<String>,
    pub msgs: i64,
    pub cost_usd: f64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub turns: Vec<TurnRow>,
    pub top_files: Vec<FileRow>,
    pub tool_counts: HashMap<String, i64>,
}

#[derive(Serialize)]
pub struct TurnRow {
    pub ts: String,
    pub cost_usd: f64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub tools: Vec<TurnTool>,
}

#[derive(Serialize)]
pub struct TurnTool {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct FileRow {
    pub file_path: String,
    pub count: i64,
}

#[derive(Serialize)]
pub struct CacheStats {
    pub messages: i64,
    pub sessions: i64,
    pub projects: i64,
    pub first_ts: Option<String>,
    pub last_ts: Option<String>,
    pub db_bytes: i64,
}

pub fn cache_stats(db: &Db) -> Result<CacheStats> {
    let (messages, sessions, projects, first_ts, last_ts): (i64, i64, i64, Option<String>, Option<String>) = db
        .conn
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT session_id), COUNT(DISTINCT project),
                    MIN(ts), MAX(ts)
             FROM messages",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    // page_count * page_size — covers main DB file, ignores WAL.
    let db_bytes: i64 = db.conn.query_row(
        "SELECT (SELECT page_count FROM pragma_page_count) * (SELECT page_size FROM pragma_page_size)",
        [],
        |r| r.get(0),
    )?;
    Ok(CacheStats { messages, sessions, projects, first_ts, last_ts, db_bytes })
}

pub fn clear_cache(db: &Db) -> Result<()> {
    db.conn.execute_batch(
        "DELETE FROM messages;
         DELETE FROM file_offsets;
         DELETE FROM dismissed_recs;
         VACUUM;",
    )?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
pub struct PricingRow {
    pub model: String,
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

pub fn pricing_table() -> Vec<PricingRow> {
    crate::pricing::current_table()
        .into_iter()
        .map(|(model, p)| PricingRow {
            model,
            input: p.input,
            output: p.output,
            cache_write: p.cache_write,
            cache_read: p.cache_read,
        })
        .collect()
}

pub fn set_pricing(db: &Db, rows: Vec<PricingRow>) -> Result<usize> {
    use crate::pricing::{save_to_db, recost_all, Price};
    let map: std::collections::HashMap<String, Price> = rows
        .into_iter()
        .map(|r| {
            (
                r.model,
                Price {
                    input: r.input,
                    output: r.output,
                    cache_write: r.cache_write,
                    cache_read: r.cache_read,
                },
            )
        })
        .collect();
    save_to_db(db, &map)?;
    recost_all(db)
}

pub fn session_detail(db: &Db, session_id: &str) -> Result<SessionDetail> {
    let mut stmt = db.conn.prepare(
        "SELECT project, MAX(model), COUNT(*), SUM(cost_usd),
                SUM(input_tok), SUM(output_tok), SUM(cache_w_tok), SUM(cache_r_tok)
         FROM messages WHERE session_id = ?1",
    )?;
    let (project, model, msgs, cost, tot_inp, tot_out, tot_cw, tot_cr): (
        String, String, i64, f64, i64, i64, i64, i64,
    ) = stmt.query_row(params![session_id], |r| {
        Ok((
            r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
            r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
        ))
    })?;

    let mut stmt = db.conn.prepare(
        "SELECT ts, cost_usd, input_tok, output_tok, cache_w_tok, cache_r_tok, tools_json
         FROM messages WHERE session_id = ?1 ORDER BY ts ASC",
    )?;
    let mut rows = stmt.query(params![session_id])?;
    let mut turns = Vec::new();
    let mut file_counts: HashMap<String, i64> = HashMap::new();
    let mut tool_counts: HashMap<String, i64> = HashMap::new();
    while let Some(r) = rows.next()? {
        let ts: String = r.get(0)?;
        let cost: f64 = r.get(1)?;
        let inp: i64 = r.get(2)?;
        let out: i64 = r.get(3)?;
        let cw: i64 = r.get(4)?;
        let cr: i64 = r.get(5)?;
        let tools_json: String = r.get(6)?;
        let mut turn_tools: HashMap<String, i64> = HashMap::new();
        if let Ok(tools) = serde_json::from_str::<Vec<serde_json::Value>>(&tools_json) {
            for t in tools {
                let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                *tool_counts.entry(name.to_string()).or_default() += 1;
                *turn_tools.entry(name.to_string()).or_default() += 1;
                if name == "Read" {
                    if let Some(fp) = t.get("file_path").and_then(|v| v.as_str()) {
                        *file_counts.entry(fp.to_string()).or_default() += 1;
                    }
                }
            }
        }
        let mut tools_vec: Vec<TurnTool> = turn_tools
            .into_iter()
            .map(|(name, count)| TurnTool { name, count })
            .collect();
        tools_vec.sort_by(|a, b| b.count.cmp(&a.count));
        turns.push(TurnRow {
            ts,
            cost_usd: cost,
            input_tok: inp,
            output_tok: out,
            cache_w_tok: cw,
            cache_r_tok: cr,
            tools: tools_vec,
        });
    }

    let mut top_files: Vec<FileRow> = file_counts
        .into_iter()
        .map(|(file_path, count)| FileRow { file_path, count })
        .collect();
    top_files.sort_by(|a, b| b.count.cmp(&a.count));
    top_files.truncate(20);

    let title: Option<String> = db
        .conn
        .query_row(
            "SELECT title FROM session_titles WHERE session_id = ?1",
            params![session_id],
            |r| r.get(0),
        )
        .ok();
    Ok(SessionDetail {
        session_id: session_id.to_string(),
        project,
        model,
        title,
        msgs,
        cost_usd: cost,
        input_tok: tot_inp,
        output_tok: tot_out,
        cache_w_tok: tot_cw,
        cache_r_tok: tot_cr,
        turns,
        top_files,
        tool_counts,
    })
}

#[derive(Serialize)]
pub struct BlockUsage {
    pub active: bool,
    pub block_start: String,
    pub block_end: String,
    pub now: String,
    pub seconds_remaining: i64,
    pub tokens: i64,
    pub input_tok: i64,
    pub output_tok: i64,
    pub cache_w_tok: i64,
    pub cache_r_tok: i64,
    pub cost_usd: f64,
    pub msgs: i64,
    pub limit_tokens: i64,
    pub limit_source: String, // "manual" | "auto" | "none"
    pub auto_limit_tokens: i64,
    pub historical_p50: i64,
    pub historical_p90: i64,
    pub historical_max: i64,
    pub historical_blocks: i64,
}

const BLOCK_HOURS: i64 = 5;
const HISTORICAL_WINDOW_DAYS: i64 = 30;

/// Walk recent messages to bucket them into 5h Claude blocks; return total tokens
/// per block (oldest first). Used for both the current-block calc and historical stats.
fn collect_blocks(db: &Db, since: &str) -> Result<Vec<(DateTime<Utc>, i64, i64, i64, i64, f64, i64)>> {
    let mut stmt = db.conn.prepare(
        "SELECT ts, input_tok, output_tok, cache_w_tok, cache_r_tok, cost_usd
         FROM messages WHERE ts >= ?1 ORDER BY ts ASC",
    )?;
    let rows = stmt
        .query_map(params![since], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, f64>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let block_dur = Duration::hours(BLOCK_HOURS);
    let mut blocks: Vec<(DateTime<Utc>, i64, i64, i64, i64, f64, i64)> = Vec::new();
    let mut cur: Option<(DateTime<Utc>, i64, i64, i64, i64, f64, i64)> = None;
    for (ts, i, o, w, r, c) in rows {
        let Ok(dt) = DateTime::parse_from_rfc3339(&ts) else { continue };
        let dt = dt.with_timezone(&Utc);
        match cur {
            None => cur = Some((dt, i, o, w, r, c, 1)),
            Some((bs, ai, ao, aw, ar, ac, am)) => {
                if dt > bs + block_dur {
                    blocks.push((bs, ai, ao, aw, ar, ac, am));
                    cur = Some((dt, i, o, w, r, c, 1));
                } else {
                    cur = Some((bs, ai + i, ao + o, aw + w, ar + r, ac + c, am + 1));
                }
            }
        }
    }
    if let Some(b) = cur {
        blocks.push(b);
    }
    Ok(blocks)
}

fn percentile_sorted(sorted: &[i64], pct: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() - 1) as f64 * pct).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

pub fn block_usage(db: &Db) -> Result<BlockUsage> {
    let now = Utc::now();
    // Pull the historical window in one shot so we can compute both the current block
    // and the historical baseline without two passes through the index.
    let history_cutoff = (now - Duration::days(HISTORICAL_WINDOW_DAYS)).to_rfc3339();
    let blocks = collect_blocks(db, &history_cutoff)?;

    // Block sums (input+output+cache_w+cache_r) used for the historical baseline.
    // Exclude the latest block if it's still active so we don't bias the baseline downward.
    let block_dur = Duration::hours(BLOCK_HOURS);
    let mut totals: Vec<i64> = blocks
        .iter()
        .filter(|(bs, _, _, _, _, _, _)| now >= *bs + block_dur)
        .map(|(_, i, o, w, r, _, _)| i + o + w + r)
        .collect();
    totals.sort_unstable();
    let historical_blocks = totals.len() as i64;
    let historical_p50 = percentile_sorted(&totals, 0.5);
    let historical_p90 = percentile_sorted(&totals, 0.9);
    let historical_max = *totals.last().unwrap_or(&0);
    // Use historical max as the auto-detected ceiling; it represents the highest the
    // user has pushed a block without (presumably) being throttled.
    let auto_limit_tokens = historical_max;

    let manual_limit = get_setting_i64(db, "block_limit_tokens").unwrap_or(0);
    let (limit_tokens, limit_source) = if manual_limit > 0 {
        (manual_limit, "manual".to_string())
    } else if auto_limit_tokens > 0 {
        (auto_limit_tokens, "auto".to_string())
    } else {
        (0, "none".to_string())
    };

    let Some(&(bs, input, output, cw, cr, cost, msgs)) = blocks.last() else {
        return Ok(BlockUsage {
            active: false,
            block_start: String::new(),
            block_end: String::new(),
            now: now.to_rfc3339(),
            seconds_remaining: 0,
            tokens: 0,
            input_tok: 0,
            output_tok: 0,
            cache_w_tok: 0,
            cache_r_tok: 0,
            cost_usd: 0.0,
            msgs: 0,
            limit_tokens,
            limit_source,
            auto_limit_tokens,
            historical_p50,
            historical_p90,
            historical_max,
            historical_blocks,
        });
    };
    let be = bs + block_dur;
    let active = now < be;
    let seconds_remaining = (be - now).num_seconds().max(0);
    Ok(BlockUsage {
        active,
        block_start: bs.to_rfc3339(),
        block_end: be.to_rfc3339(),
        now: now.to_rfc3339(),
        seconds_remaining,
        tokens: input + output + cw + cr,
        input_tok: input,
        output_tok: output,
        cache_w_tok: cw,
        cache_r_tok: cr,
        cost_usd: cost,
        msgs,
        limit_tokens,
        limit_source,
        auto_limit_tokens,
        historical_p50,
        historical_p90,
        historical_max,
        historical_blocks,
    })
}

pub fn get_setting(db: &Db, key: &str) -> Result<Option<String>> {
    let mut stmt = db
        .conn
        .prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row.get(0)?))
    } else {
        Ok(None)
    }
}

pub fn set_setting(db: &Db, key: &str, value: &str) -> Result<()> {
    db.conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn get_setting_i64(db: &Db, key: &str) -> Option<i64> {
    get_setting(db, key).ok().flatten().and_then(|v| v.parse().ok())
}

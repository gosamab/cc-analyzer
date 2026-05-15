use anyhow::Result;
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
}

pub fn sessions(db: &Db, since: Option<&str>, until: Option<&str>) -> Result<Vec<SessionRow>> {
    let (where_sql, args) = build_where(since, until, None);
    let q = format!(
        "SELECT session_id, project, MAX(model), COUNT(*),
                SUM(input_tok), SUM(output_tok), SUM(cache_w_tok), SUM(cache_r_tok),
                SUM(cost_usd), MIN(ts), MAX(ts)
         FROM messages {where_sql}
         GROUP BY session_id, project
         ORDER BY SUM(input_tok+output_tok+cache_w_tok+cache_r_tok) DESC
         LIMIT 500"
    );
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

    // Rule 5 — Bash overuse (sessions where Bash dominates the tool mix, often
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
                    let key: String = cmd
                        .split_whitespace()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" ");
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

    Ok(out)
}

#[derive(Serialize)]
pub struct SessionDetail {
    pub session_id: String,
    pub project: String,
    pub model: String,
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

    Ok(SessionDetail {
        session_id: session_id.to_string(),
        project,
        model,
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

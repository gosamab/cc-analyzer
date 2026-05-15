// USD per 1M tokens. Defaults below; overrides persisted in the `settings` table
// and loaded into PRICING at startup / on save.

use anyhow::Result;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::db::Db;

#[derive(Debug, Clone, Copy)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write: f64,
    pub cache_read: f64,
}

pub const FAMILIES: &[&str] = &["opus", "sonnet", "haiku"];

fn default_for(family: &str) -> Price {
    match family {
        "opus" => Price { input: 15.0, output: 75.0, cache_write: 18.75, cache_read: 1.50 },
        "sonnet" => Price { input: 3.0, output: 15.0, cache_write: 3.75, cache_read: 0.30 },
        "haiku" => Price { input: 0.80, output: 4.0, cache_write: 1.00, cache_read: 0.08 },
        // Unknown model → Sonnet rates, avoids wild overestimates.
        _ => Price { input: 3.0, output: 15.0, cache_write: 3.75, cache_read: 0.30 },
    }
}

static PRICING: RwLock<Option<HashMap<String, Price>>> = RwLock::new(None);

fn family_of(model: &str) -> &'static str {
    for f in FAMILIES {
        if model.contains(f) {
            return f;
        }
    }
    "sonnet"
}

fn ensure_loaded() {
    if PRICING.read().unwrap().is_some() {
        return;
    }
    let mut map = HashMap::new();
    for f in FAMILIES {
        map.insert((*f).to_string(), default_for(f));
    }
    *PRICING.write().unwrap() = Some(map);
}

pub fn price_for(model: &str) -> Price {
    ensure_loaded();
    let family = family_of(model);
    let guard = PRICING.read().unwrap();
    guard
        .as_ref()
        .and_then(|m| m.get(family).copied())
        .unwrap_or_else(|| default_for(family))
}

pub fn cost_usd(model: &str, inp: u64, out: u64, cw: u64, cr: u64) -> f64 {
    let p = price_for(model);
    (inp as f64 * p.input
        + out as f64 * p.output
        + cw as f64 * p.cache_write
        + cr as f64 * p.cache_read)
        / 1_000_000.0
}

pub fn current_table() -> Vec<(String, Price)> {
    ensure_loaded();
    let guard = PRICING.read().unwrap();
    let map = guard.as_ref().expect("pricing loaded");
    FAMILIES
        .iter()
        .map(|f| ((*f).to_string(), *map.get(*f).unwrap()))
        .collect()
}

pub fn load_from_db(db: &Db) -> Result<()> {
    ensure_loaded();
    let mut stmt = db
        .conn
        .prepare("SELECT value FROM settings WHERE key = 'pricing'")?;
    let mut rows = stmt.query([])?;
    let Some(r) = rows.next()? else { return Ok(()) };
    let json: String = r.get(0)?;
    let parsed: HashMap<String, Price> = match serde_json::from_str::<HashMap<String, serde_json::Value>>(&json) {
        Ok(raw) => raw
            .into_iter()
            .filter_map(|(k, v)| {
                let input = v.get("input")?.as_f64()?;
                let output = v.get("output")?.as_f64()?;
                let cache_write = v.get("cache_write")?.as_f64()?;
                let cache_read = v.get("cache_read")?.as_f64()?;
                Some((k, Price { input, output, cache_write, cache_read }))
            })
            .collect(),
        Err(_) => return Ok(()),
    };
    let mut guard = PRICING.write().unwrap();
    let map = guard.as_mut().unwrap();
    for (k, v) in parsed {
        map.insert(k, v);
    }
    Ok(())
}

pub fn save_to_db(db: &Db, table: &HashMap<String, Price>) -> Result<()> {
    let serializable: HashMap<&String, serde_json::Value> = table
        .iter()
        .map(|(k, p)| {
            (
                k,
                serde_json::json!({
                    "input": p.input,
                    "output": p.output,
                    "cache_write": p.cache_write,
                    "cache_read": p.cache_read,
                }),
            )
        })
        .collect();
    let json = serde_json::to_string(&serializable)?;
    db.conn.execute(
        "INSERT INTO settings(key, value) VALUES('pricing', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![json],
    )?;
    // Swap into the in-memory cache.
    let mut guard = PRICING.write().unwrap();
    let map = guard.as_mut().unwrap();
    for (k, v) in table {
        map.insert(k.clone(), *v);
    }
    Ok(())
}

/// Re-cost every message row using current pricing. Run after save_to_db.
pub fn recost_all(db: &Db) -> Result<usize> {
    let tx = db.conn.unchecked_transaction()?;
    let mut select = tx.prepare(
        "SELECT id, model, input_tok, output_tok, cache_w_tok, cache_r_tok FROM messages",
    )?;
    let rows: Vec<(i64, String, i64, i64, i64, i64)> = select
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?
        .filter_map(|r| r.ok())
        .collect();
    drop(select);
    let mut update = tx.prepare("UPDATE messages SET cost_usd = ?1 WHERE id = ?2")?;
    let mut n = 0usize;
    for (id, model, inp, out, cw, cr) in rows {
        let c = cost_usd(&model, inp as u64, out as u64, cw as u64, cr as u64);
        update.execute(params![c, id])?;
        n += 1;
    }
    drop(update);
    tx.commit()?;
    Ok(n)
}

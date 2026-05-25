use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::PathBuf;

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open() -> Result<Self> {
        let path = cache_path();
        std::fs::create_dir_all(path.parent().unwrap())?;
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA temp_store=MEMORY;",
        )?;
        Ok(Self { conn })
    }

    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id           INTEGER PRIMARY KEY,
                session_id   TEXT NOT NULL,
                project      TEXT NOT NULL,
                ts           TEXT NOT NULL,
                model        TEXT,
                input_tok    INTEGER DEFAULT 0,
                output_tok   INTEGER DEFAULT 0,
                cache_w_tok  INTEGER DEFAULT 0,
                cache_r_tok  INTEGER DEFAULT 0,
                cost_usd     REAL DEFAULT 0,
                tools_json   TEXT,
                UNIQUE(session_id, ts, id)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_messages_project ON messages(project);
            CREATE INDEX IF NOT EXISTS idx_messages_ts ON messages(ts);
            -- Covering-ish index for the hot path: range filter + group by project.
            CREATE INDEX IF NOT EXISTS idx_messages_ts_project ON messages(ts, project);
            CREATE INDEX IF NOT EXISTS idx_messages_session_ts ON messages(session_id, ts);

            CREATE TABLE IF NOT EXISTS file_offsets (
                path     TEXT PRIMARY KEY,
                byte_off INTEGER NOT NULL,
                mtime    REAL
            );

            CREATE TABLE IF NOT EXISTS dismissed_recs (
                rec_key      TEXT PRIMARY KEY,
                dismissed_ts TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS session_titles (
                session_id TEXT PRIMARY KEY,
                title      TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS slash_commands (
                id         INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                project    TEXT NOT NULL,
                ts         TEXT NOT NULL,
                cmd        TEXT NOT NULL,
                UNIQUE(session_id, ts, cmd)
            );
            CREATE INDEX IF NOT EXISTS idx_slash_ts ON slash_commands(ts);
            CREATE INDEX IF NOT EXISTS idx_slash_session ON slash_commands(session_id);
            "#,
        )?;
        // First time session_titles is empty but we already have messages: reset
        // file offsets so the next refresh re-scans files for ai-title lines.
        // INSERT OR IGNORE on messages makes the re-scan a no-op for rows we have.
        let need_backfill: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM messages LIMIT 1)
                  AND NOT EXISTS(SELECT 1 FROM session_titles LIMIT 1)",
            [],
            |r| r.get(0),
        )?;
        if need_backfill {
            self.conn.execute("DELETE FROM file_offsets", [])?;
        }

        // tools_schema_version: bump when the tools_json shape changes so we
        // re-parse historical JSONLs to backfill (skill identity, slash commands).
        // Bumping this triggers a one-shot wipe of messages + offsets + slash_commands
        // and the next refresh re-ingests everything.
        const CURRENT_TOOLS_SCHEMA: i64 = 2;
        let stored: i64 = self
            .conn
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM settings WHERE key = 'tools_schema_version'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if stored < CURRENT_TOOLS_SCHEMA {
            self.conn.execute_batch(
                "DELETE FROM messages;
                 DELETE FROM file_offsets;
                 DELETE FROM slash_commands;",
            )?;
            self.conn.execute(
                "INSERT INTO settings(key, value) VALUES('tools_schema_version', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![CURRENT_TOOLS_SCHEMA.to_string()],
            )?;
        }
        Ok(())
    }

    pub fn insert_slash_command(
        &self,
        session_id: &str,
        project: &str,
        ts: &str,
        cmd: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO slash_commands(session_id, project, ts, cmd)
             VALUES(?1, ?2, ?3, ?4)",
            params![session_id, project, ts, cmd],
        )?;
        Ok(())
    }

    pub fn upsert_title(&self, session_id: &str, title: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO session_titles(session_id, title) VALUES(?1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET title = excluded.title",
            params![session_id, title],
        )?;
        Ok(())
    }

    pub fn get_offset(&self, path: &str) -> Result<u64> {
        let mut stmt = self
            .conn
            .prepare("SELECT byte_off FROM file_offsets WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            Ok(row.get::<_, i64>(0)? as u64)
        } else {
            Ok(0)
        }
    }

    pub fn set_offset(&self, path: &str, byte_off: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO file_offsets(path, byte_off) VALUES(?1, ?2)
             ON CONFLICT(path) DO UPDATE SET byte_off = excluded.byte_off",
            params![path, byte_off as i64],
        )?;
        Ok(())
    }
}

fn cache_path() -> PathBuf {
    let base = dirs::data_dir().expect("no data dir");
    base.join("cc-analyzer").join("cache.db")
}

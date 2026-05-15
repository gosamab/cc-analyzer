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
            "#,
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

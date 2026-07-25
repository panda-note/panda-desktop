//! Local SQLite cache for memo drafts and summaries.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use panda_core::{ActiveMemo, MemoSummary, SyncState, Todo, TodoOperation, TodoOperationKind};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, params};

pub struct LocalStore {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl LocalStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path).with_context(|| format!("open {}", path.display()))?;
        let store = Self {
            conn: Mutex::new(conn),
            path,
        };
        store.migrate()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS memo_cache (
                id TEXT PRIMARY KEY,
                notebook_id TEXT NOT NULL,
                title TEXT,
                excerpt TEXT NOT NULL DEFAULT '',
                tags_json TEXT NOT NULL DEFAULT '[]',
                is_pinned INTEGER NOT NULL DEFAULT 0,
                is_archived INTEGER NOT NULL DEFAULT 0,
                is_deleted INTEGER NOT NULL DEFAULT 0,
                revision INTEGER NOT NULL DEFAULT 0,
                content_hash TEXT NOT NULL DEFAULT '',
                etag TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT '',
                updated_at TEXT NOT NULL DEFAULT '',
                deleted_at TEXT,
                markdown TEXT NOT NULL DEFAULT '',
                lease_id TEXT,
                sync_state TEXT NOT NULL DEFAULT 'synced'
            );
            CREATE TABLE IF NOT EXISTS todo_cache (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, note TEXT NOT NULL DEFAULT '',
                status TEXT NOT NULL DEFAULT 'inbox', due_date TEXT, priority INTEGER NOT NULL DEFAULT 0,
                linked_memo_id TEXT, is_deleted INTEGER NOT NULL DEFAULT 0, revision INTEGER NOT NULL DEFAULT 0,
                etag TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT '', updated_at TEXT NOT NULL DEFAULT '',
                completed_at TEXT, deleted_at TEXT, sync_state TEXT NOT NULL DEFAULT 'synced'
            );
            CREATE TABLE IF NOT EXISTS todo_operations (
                operation_id TEXT PRIMARY KEY, todo_id TEXT NOT NULL, kind TEXT NOT NULL,
                payload_json TEXT NOT NULL, base_revision INTEGER, if_match_etag TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            "#,
        )?;
        Ok(())
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.conn.lock().execute(
            "INSERT INTO meta(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let v = conn
            .query_row(
                "SELECT value FROM meta WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(v)
    }

    pub fn upsert_summary(&self, summary: &MemoSummary, sync_state: SyncState) -> Result<()> {
        let tags = serde_json::to_string(&summary.tags)?;
        self.conn.lock().execute(
            r#"
            INSERT INTO memo_cache(
                id, notebook_id, title, excerpt, tags_json, is_pinned, is_archived, is_deleted,
                revision, content_hash, etag, created_at, updated_at, deleted_at, sync_state
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
            ON CONFLICT(id) DO UPDATE SET
                notebook_id=excluded.notebook_id,
                title=excluded.title,
                excerpt=excluded.excerpt,
                tags_json=excluded.tags_json,
                is_pinned=excluded.is_pinned,
                is_archived=excluded.is_archived,
                is_deleted=excluded.is_deleted,
                revision=excluded.revision,
                content_hash=excluded.content_hash,
                etag=excluded.etag,
                created_at=excluded.created_at,
                updated_at=excluded.updated_at,
                deleted_at=excluded.deleted_at,
                sync_state=excluded.sync_state
            "#,
            params![
                summary.id,
                summary.notebook_id,
                summary.title,
                summary.excerpt,
                tags,
                summary.is_pinned as i32,
                summary.is_archived as i32,
                summary.is_deleted as i32,
                summary.revision as i64,
                summary.content_hash,
                summary.etag,
                summary.created_at,
                summary.updated_at,
                summary.deleted_at,
                sync_state.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn upsert_active(&self, memo: &ActiveMemo) -> Result<()> {
        self.conn.lock().execute(
            r#"
            INSERT INTO memo_cache(
                id, notebook_id, title, markdown, etag, revision, content_hash,
                lease_id, sync_state, excerpt, tags_json
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'','[]')
            ON CONFLICT(id) DO UPDATE SET
                notebook_id=excluded.notebook_id,
                title=excluded.title,
                markdown=excluded.markdown,
                etag=excluded.etag,
                revision=excluded.revision,
                content_hash=excluded.content_hash,
                lease_id=excluded.lease_id,
                sync_state=excluded.sync_state
            "#,
            params![
                memo.id,
                memo.notebook_id,
                memo.title,
                memo.markdown,
                memo.etag,
                memo.revision as i64,
                memo.content_hash,
                memo.lease_id,
                memo.sync_state.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn save_draft(&self, id: &str, markdown: &str, sync_state: SyncState) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE memo_cache SET markdown = ?2, sync_state = ?3 WHERE id = ?1",
            params![id, markdown, sync_state.as_str()],
        )?;
        Ok(())
    }

    pub fn mark_synced(
        &self,
        id: &str,
        etag: &str,
        revision: u64,
        content_hash: &str,
        lease_id: Option<&str>,
    ) -> Result<()> {
        self.conn.lock().execute(
            "UPDATE memo_cache SET etag=?2, revision=?3, content_hash=?4, lease_id=?5, sync_state='synced' WHERE id=?1",
            params![id, etag, revision as i64, content_hash, lease_id],
        )?;
        Ok(())
    }

    pub fn get_active(&self, id: &str) -> Result<Option<ActiveMemo>> {
        let conn = self.conn.lock();
        conn.query_row(
            r#"
            SELECT id, notebook_id, title, markdown, etag, revision, content_hash, lease_id, sync_state
            FROM memo_cache WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(ActiveMemo {
                    id: row.get(0)?,
                    notebook_id: row.get(1)?,
                    title: row.get(2)?,
                    tags: Vec::new(),
                    markdown: row.get(3)?,
                    etag: row.get(4)?,
                    revision: row.get::<_, i64>(5)? as u64,
                    content_hash: row.get(6)?,
                    lease_id: row.get(7)?,
                    sync_state: SyncState::parse(&row.get::<_, String>(8)?),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn list_summaries(&self) -> Result<Vec<(MemoSummary, SyncState)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, notebook_id, title, excerpt, tags_json, is_pinned, is_archived, is_deleted,
                   revision, content_hash, etag, created_at, updated_at, deleted_at, sync_state
            FROM memo_cache
            ORDER BY updated_at DESC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            let tags_json: String = row.get(4)?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            Ok((
                MemoSummary {
                    id: row.get(0)?,
                    notebook_id: row.get(1)?,
                    title: row.get(2)?,
                    excerpt: row.get(3)?,
                    tags,
                    is_pinned: row.get::<_, i32>(5)? != 0,
                    is_archived: row.get::<_, i32>(6)? != 0,
                    is_deleted: row.get::<_, i32>(7)? != 0,
                    revision: row.get::<_, i64>(8)? as u64,
                    content_hash: row.get(9)?,
                    etag: row.get(10)?,
                    created_at: row.get(11)?,
                    updated_at: row.get(12)?,
                    deleted_at: row.get(13)?,
                },
                SyncState::parse(&row.get::<_, String>(14)?),
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn upsert_todo(&self, todo: &Todo, sync_state: SyncState) -> Result<()> {
        let status = serde_json::to_string(&todo.status)?
            .trim_matches('"')
            .to_string();
        self.conn.lock().execute(r#"INSERT INTO todo_cache(id,title,note,status,due_date,priority,linked_memo_id,is_deleted,revision,etag,created_at,updated_at,completed_at,deleted_at,sync_state) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15) ON CONFLICT(id) DO UPDATE SET title=excluded.title,note=excluded.note,status=excluded.status,due_date=excluded.due_date,priority=excluded.priority,linked_memo_id=excluded.linked_memo_id,is_deleted=excluded.is_deleted,revision=excluded.revision,etag=excluded.etag,created_at=excluded.created_at,updated_at=excluded.updated_at,completed_at=excluded.completed_at,deleted_at=excluded.deleted_at,sync_state=excluded.sync_state"#, params![todo.id,todo.title,todo.note,status,todo.due_date,todo.priority,todo.linked_memo_id,todo.is_deleted as i32,todo.revision as i64,todo.etag,todo.created_at,todo.updated_at,todo.completed_at,todo.deleted_at,sync_state.as_str()])?;
        Ok(())
    }

    pub fn list_todos(&self) -> Result<Vec<(Todo, SyncState)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT id,title,note,status,due_date,priority,linked_memo_id,is_deleted,revision,etag,created_at,updated_at,completed_at,deleted_at,sync_state FROM todo_cache ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                Todo {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    note: r.get(2)?,
                    status: serde_json::from_str(&format!("\"{}\"", r.get::<_, String>(3)?))
                        .unwrap_or_default(),
                    due_date: r.get(4)?,
                    priority: r.get(5)?,
                    linked_memo_id: r.get(6)?,
                    is_deleted: r.get::<_, i32>(7)? != 0,
                    revision: r.get::<_, i64>(8)? as u64,
                    etag: r.get(9)?,
                    created_at: r.get(10)?,
                    updated_at: r.get(11)?,
                    completed_at: r.get(12)?,
                    deleted_at: r.get(13)?,
                },
                SyncState::parse(&r.get::<_, String>(14)?),
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn enqueue_todo_operation(&self, operation: &TodoOperation) -> Result<()> {
        self.conn.lock().execute("INSERT OR REPLACE INTO todo_operations(operation_id,todo_id,kind,payload_json,base_revision,if_match_etag) VALUES(?1,?2,?3,?4,?5,?6)",params![operation.operation_id,operation.todo_id,serde_json::to_string(&operation.kind)?,operation.payload_json,operation.base_revision.map(|v|v as i64),operation.if_match_etag])?;
        Ok(())
    }

    pub fn pending_todo_operations(&self) -> Result<Vec<TodoOperation>> {
        let conn = self.conn.lock();
        let mut stmt=conn.prepare("SELECT operation_id,todo_id,kind,payload_json,base_revision,if_match_etag FROM todo_operations ORDER BY created_at, operation_id")?;
        let rows = stmt.query_map([], |r| {
            let kind: String = r.get(2)?;
            Ok(TodoOperation {
                operation_id: r.get(0)?,
                todo_id: r.get(1)?,
                kind: serde_json::from_str(&kind).unwrap_or(TodoOperationKind::Update),
                payload_json: r.get(3)?,
                base_revision: r.get::<_, Option<i64>>(4)?.map(|v| v as u64),
                if_match_etag: r.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn remove_todo_operation(&self, operation_id: &str) -> Result<()> {
        self.conn.lock().execute(
            "DELETE FROM todo_operations WHERE operation_id=?1",
            params![operation_id],
        )?;
        Ok(())
    }
}

/// Placeholder for older stub links.
#[derive(Debug, Default)]
pub struct LocalStorePlaceholder;

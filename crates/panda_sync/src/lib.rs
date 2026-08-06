//! Durable Todo synchronization for the desktop client.
//!
//! The server journal is intentionally treated as an append-only cursor. A
//! local operation is written to SQLite before it is sent, and the cursor is
//! advanced only after each remote change has been applied to the cache.

use std::collections::HashSet;

use anyhow::{Context, Result, anyhow};
use panda_api::{ApiClient, SyncPushItem, SyncTodoPush};
use panda_core::{SyncState, Todo, TodoOperation, TodoOperationKind};
use panda_store::LocalStore;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub pushed: usize,
    pub pulled: usize,
    pub conflicts: usize,
}

#[derive(Clone)]
pub struct SyncEngine {
    client: ApiClient,
    store: std::sync::Arc<LocalStore>,
    device_id: String,
}

impl SyncEngine {
    pub fn new(
        client: ApiClient,
        store: std::sync::Arc<LocalStore>,
        device_id: impl Into<String>,
    ) -> Self {
        Self {
            client,
            store,
            device_id: device_id.into(),
        }
    }

    /// Replay pending operations in creation order, then pull the server
    /// journal. One operation is sent at a time so dependent edits cannot leap
    /// over an earlier revision acknowledgement.
    pub fn sync_todos(&self) -> Result<SyncReport> {
        let mut report = SyncReport::default();
        let pending = self.store.pending_todo_operations()?;
        for operation in &pending {
            let write_id = operation_id(operation);
            let todo: Todo = serde_json::from_str(&operation.payload_json)
                .with_context(|| format!("decode queued Todo {}", operation.todo_id))?;
            let op = match operation.kind {
                TodoOperationKind::Create => "todo.create",
                TodoOperationKind::Update => "todo.update",
                TodoOperationKind::Complete => "todo.complete",
                TodoOperationKind::Restore => "todo.restore",
                TodoOperationKind::Delete => "todo.delete",
            };
            let response = self.client.sync_push_blocking(
                &self.device_id,
                vec![SyncPushItem {
                    client_op_id: write_id.clone(),
                    op: op.into(),
                    todo: Some(SyncTodoPush {
                        todo,
                        // Once the predecessor for this Todo has been accepted,
                        // this full-state write is based on that acknowledged
                        // state. Keeping the original revision here would make a
                        // local toggle followed by an edit conflict with itself.
                        base_revision: operation
                            .depends_on_write_id
                            .is_none()
                            .then_some(operation.base_revision)
                            .flatten(),
                        if_match_etag: operation
                            .depends_on_write_id
                            .is_none()
                            .then(|| operation.if_match_etag.clone())
                            .flatten(),
                    }),
                    depends_on_client_op_id: operation.depends_on_write_id.clone(),
                }],
            )?;
            let result = response
                .results
                .into_iter()
                .find(|result| result.client_op_id == write_id)
                .ok_or_else(|| anyhow!("sync response omitted operation {write_id}"))?;
            if result.ok {
                if let Some(todo) = result.todo {
                    self.store.upsert_todo(&todo, SyncState::Synced)?;
                }
                self.store.remove_todo_operation(&operation.operation_id)?;
                report.pushed += 1;
            } else {
                let message = result
                    .error_code
                    .unwrap_or_else(|| "remote operation failed".into());
                self.store
                    .mark_todo_operation_failure(&operation.operation_id, &message)?;
                if message.contains("conflict") {
                    self.store
                        .set_todo_sync_state(&operation.todo_id, SyncState::Conflict)?;
                    report.conflicts += 1;
                } else {
                    self.store
                        .set_todo_sync_state(&operation.todo_id, SyncState::Failed)?;
                }
                // A dependent operation must wait for the user to resolve the
                // failed base operation; do not send it with a stale revision.
                break;
            }
        }

        let pending_ids: HashSet<String> = self
            .store
            .pending_todo_operations()?
            .into_iter()
            .map(|operation| operation.todo_id)
            .collect();
        let mut cursor = self
            .store
            .get_meta("sync_cursor")?
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0);
        loop {
            let page = self
                .client
                .sync_pull_blocking(cursor, 200, &self.device_id)?;
            for change in page.changes {
                if change.entity_type != "todo" {
                    continue;
                }
                let Some(payload) = change.payload_json else {
                    continue;
                };
                let todo: Todo = serde_json::from_str(&payload)
                    .with_context(|| format!("decode remote Todo {}", change.entity_id))?;
                if pending_ids.contains(&todo.id) {
                    self.store
                        .set_todo_sync_state(&todo.id, SyncState::Conflict)?;
                    report.conflicts += 1;
                    continue;
                }
                self.store.upsert_todo(&todo, SyncState::Synced)?;
                report.pulled += 1;
            }
            cursor = page.cursor;
            self.store.set_meta("sync_cursor", &cursor.to_string())?;
            self.store
                .set_meta("sync_epoch", &page.sync_epoch.to_string())?;
            if !page.has_more {
                break;
            }
        }
        Ok(report)
    }
}

fn operation_id(operation: &TodoOperation) -> String {
    if operation.write_id.is_empty() {
        operation.operation_id.clone()
    } else {
        operation.write_id.clone()
    }
}

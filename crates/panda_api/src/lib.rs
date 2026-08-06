//! JSON HTTP client for panda-server `/panda/v1`.

use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use panda_core::{ActiveMemo, MemoSummary, Notebook, SyncState, Todo};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("panda-api")
            .build()
            .expect("panda-api tokio runtime")
    })
}

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Default)]
pub struct ApiConfig {
    pub api_url: String,
    pub token: String,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{message}")]
    Http {
        status: u16,
        code: String,
        message: String,
        current_etag: Option<String>,
    },
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl ApiError {
    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Http { status: 409, .. })
    }

    pub fn is_revision_conflict(&self) -> bool {
        matches!(
            self,
            Self::Http {
                status: 409,
                code,
                ..
            } if code == "revision_conflict"
        )
    }

    pub fn is_lease_conflict(&self) -> bool {
        matches!(
            self,
            Self::Http {
                status: 409,
                code,
                ..
            } if code == "lease_conflict"
        )
    }

    pub fn is_content_conflict(&self) -> bool {
        matches!(
            self,
            Self::Http {
                status: 409,
                code,
                ..
            } if code == "content_conflict" || code == "conflict"
        )
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Http { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    pub fn current_etag(&self) -> Option<&str> {
        match self {
            Self::Http { current_etag, .. } => current_etag.as_deref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiClient {
    http: Client,
    base_url: String,
    token: String,
}

impl ApiClient {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
        }
    }

    pub fn with_token(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self::new(base_url, token)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn request_json<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
        body: Option<&Value>,
    ) -> Result<T, ApiError> {
        let mut req = self
            .http
            .request(method, self.url(path))
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json");
        if let Some(body) = body {
            req = req.header("Content-Type", "application/json").json(body);
        }
        let resp = req.send().await.map_err(|e| ApiError::Other(e.into()))?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(|e| ApiError::Other(e.into()))?;
        if status.is_success() {
            if bytes.is_empty() {
                // Some endpoints may return empty; caller should use unit types carefully.
                return serde_json::from_str("null")
                    .or_else(|_| serde_json::from_str("{}"))
                    .map_err(|e| ApiError::Other(e.into()));
            }
            return serde_json::from_slice(&bytes).map_err(|e| {
                ApiError::Other(anyhow!(
                    "decode {path}: {e}; body={}",
                    String::from_utf8_lossy(&bytes)
                ))
            });
        }
        let err: WireApiError = serde_json::from_slice(&bytes).unwrap_or(WireApiError {
            code: status.as_str().to_string(),
            message: String::from_utf8_lossy(&bytes).into_owned(),
            current_etag: None,
        });
        Err(ApiError::Http {
            status: status.as_u16(),
            code: err.code,
            message: err.message,
            current_etag: err.current_etag,
        })
    }

    pub async fn login(
        base_url: &str,
        username: &str,
        password: &str,
        device_id: Option<&str>,
    ) -> Result<LoginResponse, ApiError> {
        let client = Client::new();
        let base = base_url.trim_end_matches('/');
        let mut body = serde_json::json!({
            "username": username,
            "password": password,
        });
        if let Some(device_id) = device_id {
            body["device_id"] = Value::String(device_id.to_string());
        }
        let resp = client
            .post(format!("{base}/auth/login"))
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Other(e.into()))?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(|e| ApiError::Other(e.into()))?;
        if !status.is_success() {
            let err: WireApiError = serde_json::from_slice(&bytes).unwrap_or(WireApiError {
                code: "login_failed".into(),
                message: String::from_utf8_lossy(&bytes).into_owned(),
                current_etag: None,
            });
            return Err(ApiError::Http {
                status: status.as_u16(),
                code: err.code,
                message: err.message,
                current_etag: err.current_etag,
            });
        }
        serde_json::from_slice(&bytes).map_err(|e| ApiError::Other(e.into()))
    }

    pub async fn create_api_token(
        base_url: &str,
        session_token: &str,
        name: &str,
    ) -> Result<ApiTokenInfo, ApiError> {
        let client = ApiClient::new(base_url, session_token);
        let body = serde_json::json!({
            "name": name,
            "scopes": [],
        });
        client
            .request_json(
                reqwest::Method::POST,
                "/api-tokens",
                session_token,
                Some(&body),
            )
            .await
    }

    /// Login and mint a long-lived API token for desktop use.
    pub async fn bootstrap_token(
        base_url: &str,
        username: &str,
        password: &str,
        device_id: &str,
        token_name: &str,
    ) -> Result<String, ApiError> {
        let login = Self::login(base_url, username, password, Some(device_id)).await?;
        let info = Self::create_api_token(base_url, &login.session_token, token_name).await?;
        info.token
            .ok_or_else(|| ApiError::Other(anyhow!("server did not return API token plaintext")))
    }

    pub async fn list_notebooks(&self) -> Result<Vec<Notebook>, ApiError> {
        let resp: NotebookListResponse = self
            .request_json(reqwest::Method::GET, "/notebooks", &self.token, None)
            .await?;
        Ok(resp.items)
    }

    pub async fn list_memos(
        &self,
        notebook_id: Option<&str>,
        trash: bool,
        q: Option<&str>,
        limit: i64,
    ) -> Result<Vec<MemoSummary>, ApiError> {
        let mut path = format!("/memos?limit={limit}&trash={trash}");
        if let Some(id) = notebook_id {
            path.push_str(&format!("&notebook_id={id}"));
        }
        if let Some(q) = q {
            path.push_str(&format!("&q={}", urlencoding_lite(q)));
        }
        let resp: MemoListResponse = self
            .request_json(reqwest::Method::GET, &path, &self.token, None)
            .await?;
        Ok(resp.items)
    }

    pub async fn list_todos(&self, filter: &str, limit: i64) -> Result<Vec<Todo>, ApiError> {
        let resp: TodoListResponse = self
            .request_json(
                reqwest::Method::GET,
                &format!("/todos?filter={filter}&limit={}", limit.clamp(1, 200)),
                &self.token,
                None,
            )
            .await?;
        Ok(resp.items)
    }

    pub async fn create_todo(&self, title: &str) -> Result<Todo, ApiError> {
        let body = serde_json::json!({"title": title});
        self.request_json(reqwest::Method::POST, "/todos", &self.token, Some(&body))
            .await
    }

    pub async fn update_todo(&self, todo: &Todo) -> Result<Todo, ApiError> {
        let body = serde_json::json!({"title":todo.title,"note":todo.note,"status":todo.status,"due_date":todo.due_date,"priority":todo.priority,"linked_memo_id":todo.linked_memo_id,"base_revision":todo.revision,"if_match_etag":todo.etag});
        self.request_json(
            reqwest::Method::PATCH,
            &format!("/todos/{}", todo.id),
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn set_todo_completed(&self, todo: &Todo, completed: bool) -> Result<Todo, ApiError> {
        let body = serde_json::json!({"completed":completed,"base_revision":todo.revision,"if_match_etag":todo.etag});
        self.request_json(
            reqwest::Method::POST,
            &format!("/todos/{}/complete", todo.id),
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn delete_todo(&self, todo: &Todo, permanent: bool) -> Result<(), ApiError> {
        let _: serde_json::Value = self
            .request_json(
                reqwest::Method::DELETE,
                &format!(
                    "/todos/{}?base_revision={}&if_match_etag={}&permanent={permanent}",
                    todo.id, todo.revision, todo.etag
                ),
                &self.token,
                None,
            )
            .await?;
        Ok(())
    }

    pub async fn restore_todo(&self, todo: &Todo) -> Result<Todo, ApiError> {
        let body = serde_json::json!({"base_revision":todo.revision,"if_match_etag":todo.etag});
        self.request_json(
            reqwest::Method::POST,
            &format!("/todos/{}/restore", todo.id),
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn open_memo(&self, id: &str) -> Result<ActiveMemo, ApiError> {
        let body = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "lease_mode": "soft",
        });
        let resp: MemoOpenResponse = self
            .request_json(
                reqwest::Method::POST,
                &format!("/memos/{id}/open"),
                &self.token,
                Some(&body),
            )
            .await?;
        let memo = resp
            .memo
            .ok_or_else(|| ApiError::Other(anyhow!("open response missing memo")))?;
        let summary = memo
            .summary
            .ok_or_else(|| ApiError::Other(anyhow!("open response missing summary")))?;
        let content = memo
            .content
            .ok_or_else(|| ApiError::Other(anyhow!("open response missing content")))?;
        let markdown = match content.body {
            Some(MemoContentBodyWire::Markdown(s)) => s,
            Some(MemoContentBodyWire::ContentRef(hash)) => self.get_content(&hash).await?,
            None => String::new(),
        };
        Ok(ActiveMemo {
            id: summary.id,
            notebook_id: summary.notebook_id,
            title: summary.title,
            tags: summary.tags,
            markdown,
            etag: resp.etag,
            revision: summary.revision,
            content_hash: summary.content_hash,
            lease_id: resp.lease.map(|l| l.id),
            sync_state: SyncState::Synced,
        })
    }

    pub async fn get_content(&self, hash: &str) -> Result<String, ApiError> {
        let resp = self
            .http
            .get(self.url(&format!("/contents/{hash}")))
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
            .map_err(|e| ApiError::Other(e.into()))?;
        let status = resp.status();
        let bytes = resp.bytes().await.map_err(|e| ApiError::Other(e.into()))?;
        if status != StatusCode::OK {
            return Err(ApiError::Http {
                status: status.as_u16(),
                code: "content_fetch_failed".into(),
                message: String::from_utf8_lossy(&bytes).into_owned(),
                current_etag: None,
            });
        }
        String::from_utf8(bytes.to_vec()).map_err(|e| ApiError::Other(e.into()))
    }

    pub async fn save_memo(
        &self,
        id: &str,
        markdown: &str,
        etag: &str,
        revision: u64,
        content_hash: &str,
        lease_id: Option<&str>,
        title: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<MemoSaveAck, ApiError> {
        let mut body = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "if_match_etag": etag,
            "base_revision": revision,
            "base_content_hash": content_hash,
            "idempotency_key": Uuid::new_v4().to_string(),
            "markdown_full": markdown,
        });
        if let Some(lease_id) = lease_id {
            body["lease_id"] = Value::String(lease_id.to_string());
        }
        if let Some(title) = title {
            body["title"] = Value::String(title.to_string());
        }
        if let Some(tags) = tags {
            body["tags"] = serde_json::to_value(tags).unwrap_or_else(|_| Value::Array(Vec::new()));
        }
        self.request_json(
            reqwest::Method::POST,
            &format!("/memos/{id}/save"),
            &self.token,
            Some(&body),
        )
        .await
    }

    /// Meta-only pin/unpin without rewriting markdown.
    pub async fn set_memo_pinned(
        &self,
        id: &str,
        etag: &str,
        revision: u64,
        content_hash: &str,
        is_pinned: bool,
    ) -> Result<MemoSaveAck, ApiError> {
        let body = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "if_match_etag": etag,
            "base_revision": revision,
            "base_content_hash": content_hash,
            "idempotency_key": Uuid::new_v4().to_string(),
            "is_pinned": is_pinned,
        });
        self.request_json(
            reqwest::Method::POST,
            &format!("/memos/{id}/save"),
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn create_memo(
        &self,
        notebook_id: &str,
        title: Option<&str>,
        markdown: &str,
    ) -> Result<ActiveMemo, ApiError> {
        let body = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "notebook_id": notebook_id,
            "title": title,
            "markdown": markdown,
            "tags": [],
        });
        let detail: MemoDetailWire = self
            .request_json(reqwest::Method::POST, "/memos", &self.token, Some(&body))
            .await?;
        let summary = detail
            .summary
            .ok_or_else(|| ApiError::Other(anyhow!("create missing summary")))?;
        let content = detail.content;
        let markdown = match content.and_then(|c| c.body) {
            Some(MemoContentBodyWire::Markdown(s)) => s,
            Some(MemoContentBodyWire::ContentRef(hash)) => self.get_content(&hash).await?,
            None => markdown.to_string(),
        };
        Ok(ActiveMemo {
            id: summary.id.clone(),
            notebook_id: summary.notebook_id.clone(),
            title: summary.title.clone(),
            tags: summary.tags.clone(),
            markdown,
            etag: summary.etag.clone(),
            revision: summary.revision,
            content_hash: summary.content_hash.clone(),
            lease_id: None,
            sync_state: SyncState::Synced,
        })
    }

    pub async fn delete_memo(&self, id: &str, permanent: bool) -> Result<(), ApiError> {
        let path = format!("/memos/{id}?permanent={permanent}");
        let resp = self
            .http
            .delete(self.url(&path))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::Other(e.into()))?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NO_CONTENT {
            return Ok(());
        }
        let bytes = resp.bytes().await.map_err(|e| ApiError::Other(e.into()))?;
        let err: WireApiError = serde_json::from_slice(&bytes).unwrap_or(WireApiError {
            code: "delete_failed".into(),
            message: String::from_utf8_lossy(&bytes).into_owned(),
            current_etag: None,
        });
        Err(ApiError::Http {
            status: status.as_u16(),
            code: err.code,
            message: err.message,
            current_etag: err.current_etag,
        })
    }

    pub async fn batch_move_memos(
        &self,
        memo_ids: &[String],
        notebook_id: &str,
    ) -> Result<u64, ApiError> {
        let body = serde_json::json!({
            "memo_ids": memo_ids,
            "notebook_id": notebook_id,
        });
        let resp: BatchMoveResponse = self
            .request_json(
                reqwest::Method::POST,
                "/memos/batch/move",
                &self.token,
                Some(&body),
            )
            .await?;
        Ok(resp.moved)
    }

    pub async fn create_notebook(
        &self,
        name: &str,
        parent_id: Option<&str>,
        sort_order: Option<i32>,
    ) -> Result<Notebook, ApiError> {
        let mut body = serde_json::json!({ "name": name });
        if let Some(parent_id) = parent_id {
            body["parent_id"] = Value::String(parent_id.to_string());
        }
        if let Some(sort_order) = sort_order {
            body["sort_order"] = Value::Number(sort_order.into());
        }
        self.request_json(
            reqwest::Method::POST,
            "/notebooks",
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn rename_notebook(&self, id: &str, name: &str) -> Result<Notebook, ApiError> {
        let body = serde_json::json!({ "name": name });
        self.request_json(
            reqwest::Method::PATCH,
            &format!("/notebooks/{id}"),
            &self.token,
            Some(&body),
        )
        .await
    }

    pub async fn reorder_notebooks(
        &self,
        parent_id: Option<&str>,
        notebook_ids: &[String],
    ) -> Result<Vec<Notebook>, ApiError> {
        let mut body = serde_json::json!({ "notebook_ids": notebook_ids });
        if let Some(parent_id) = parent_id {
            body["parent_id"] = Value::String(parent_id.to_string());
        }
        let resp: NotebookListResponse = self
            .request_json(
                reqwest::Method::POST,
                "/notebooks/reorder",
                &self.token,
                Some(&body),
            )
            .await?;
        Ok(resp.items)
    }

    pub async fn delete_notebook(&self, id: &str) -> Result<(), ApiError> {
        let resp = self
            .http
            .delete(self.url(&format!("/notebooks/{id}")))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError::Other(e.into()))?;
        let status = resp.status();
        if status.is_success() || status == StatusCode::NO_CONTENT {
            return Ok(());
        }
        let bytes = resp.bytes().await.map_err(|e| ApiError::Other(e.into()))?;
        let err: WireApiError = serde_json::from_slice(&bytes).unwrap_or(WireApiError {
            code: "delete_notebook_failed".into(),
            message: String::from_utf8_lossy(&bytes).into_owned(),
            current_etag: None,
        });
        Err(ApiError::Http {
            status: status.as_u16(),
            code: err.code,
            message: err.message,
            current_etag: err.current_etag,
        })
    }

    // --- blocking helpers for GPUI background tasks ---

    pub fn bootstrap_token_blocking(
        base_url: &str,
        username: &str,
        password: &str,
        device_id: &str,
        token_name: &str,
    ) -> Result<String, ApiError> {
        runtime().block_on(Self::bootstrap_token(
            base_url, username, password, device_id, token_name,
        ))
    }

    pub fn list_notebooks_blocking(&self) -> Result<Vec<Notebook>, ApiError> {
        runtime().block_on(self.list_notebooks())
    }

    pub fn list_memos_blocking(
        &self,
        notebook_id: Option<&str>,
        trash: bool,
        q: Option<&str>,
        limit: i64,
    ) -> Result<Vec<MemoSummary>, ApiError> {
        runtime().block_on(self.list_memos(notebook_id, trash, q, limit))
    }

    pub fn list_todos_blocking(&self, filter: &str, limit: i64) -> Result<Vec<Todo>, ApiError> {
        runtime().block_on(self.list_todos(filter, limit))
    }
    pub fn create_todo_blocking(&self, title: &str) -> Result<Todo, ApiError> {
        runtime().block_on(self.create_todo(title))
    }
    pub fn update_todo_blocking(&self, todo: &Todo) -> Result<Todo, ApiError> {
        runtime().block_on(self.update_todo(todo))
    }
    pub fn set_todo_completed_blocking(
        &self,
        todo: &Todo,
        completed: bool,
    ) -> Result<Todo, ApiError> {
        runtime().block_on(self.set_todo_completed(todo, completed))
    }
    pub fn delete_todo_blocking(&self, todo: &Todo, permanent: bool) -> Result<(), ApiError> {
        runtime().block_on(self.delete_todo(todo, permanent))
    }

    pub fn restore_todo_blocking(&self, todo: &Todo) -> Result<Todo, ApiError> {
        runtime().block_on(self.restore_todo(todo))
    }

    pub fn open_memo_blocking(&self, id: &str) -> Result<ActiveMemo, ApiError> {
        runtime().block_on(self.open_memo(id))
    }

    pub fn save_memo_blocking(
        &self,
        id: &str,
        markdown: &str,
        etag: &str,
        revision: u64,
        content_hash: &str,
        lease_id: Option<&str>,
        title: Option<&str>,
        tags: Option<&[String]>,
    ) -> Result<MemoSaveAck, ApiError> {
        runtime().block_on(self.save_memo(
            id,
            markdown,
            etag,
            revision,
            content_hash,
            lease_id,
            title,
            tags,
        ))
    }

    pub fn create_memo_blocking(
        &self,
        notebook_id: &str,
        title: Option<&str>,
        markdown: &str,
    ) -> Result<ActiveMemo, ApiError> {
        runtime().block_on(self.create_memo(notebook_id, title, markdown))
    }

    pub fn delete_memo_blocking(&self, id: &str, permanent: bool) -> Result<(), ApiError> {
        runtime().block_on(self.delete_memo(id, permanent))
    }

    pub fn batch_move_memos_blocking(
        &self,
        memo_ids: &[String],
        notebook_id: &str,
    ) -> Result<u64, ApiError> {
        runtime().block_on(self.batch_move_memos(memo_ids, notebook_id))
    }

    pub fn create_notebook_blocking(
        &self,
        name: &str,
        parent_id: Option<&str>,
        sort_order: Option<i32>,
    ) -> Result<Notebook, ApiError> {
        runtime().block_on(self.create_notebook(name, parent_id, sort_order))
    }

    pub fn rename_notebook_blocking(&self, id: &str, name: &str) -> Result<Notebook, ApiError> {
        runtime().block_on(self.rename_notebook(id, name))
    }

    pub fn reorder_notebooks_blocking(
        &self,
        parent_id: Option<&str>,
        notebook_ids: &[String],
    ) -> Result<Vec<Notebook>, ApiError> {
        runtime().block_on(self.reorder_notebooks(parent_id, notebook_ids))
    }

    pub fn delete_notebook_blocking(&self, id: &str) -> Result<(), ApiError> {
        runtime().block_on(self.delete_notebook(id))
    }

    pub fn set_memo_pinned_blocking(
        &self,
        id: &str,
        etag: &str,
        revision: u64,
        content_hash: &str,
        is_pinned: bool,
    ) -> Result<MemoSaveAck, ApiError> {
        runtime().block_on(self.set_memo_pinned(id, etag, revision, content_hash, is_pinned))
    }

    pub async fn list_tags(&self) -> Result<Vec<String>, ApiError> {
        let resp: TagListResponse = self
            .request_json(reqwest::Method::GET, "/tags", &self.token, None)
            .await?;
        Ok(resp.items.into_iter().map(|item| item.tag).collect())
    }

    pub fn list_tags_blocking(&self) -> Result<Vec<String>, ApiError> {
        runtime().block_on(self.list_tags())
    }

    /// Pull the append-only server journal. The cursor is advanced only after
    /// the caller has durably applied the returned changes.
    pub async fn sync_pull(
        &self,
        cursor: u64,
        limit: i64,
        device_id: &str,
    ) -> Result<SyncPullResponse, ApiError> {
        self.request_json(
            reqwest::Method::GET,
            &format!(
                "/sync/pull?cursor={cursor}&limit={}&device_id={}",
                limit.clamp(1, 500),
                urlencoding_lite(device_id),
            ),
            &self.token,
            None,
        )
        .await
    }

    pub async fn sync_push(
        &self,
        device_id: &str,
        items: Vec<SyncPushItem>,
    ) -> Result<SyncPushResponse, ApiError> {
        let body = serde_json::json!({
            "protocol_version": PROTOCOL_VERSION,
            "device_id": device_id,
            "items": items,
        });
        self.request_json(
            reqwest::Method::POST,
            "/sync/push",
            &self.token,
            Some(&body),
        )
        .await
    }

    pub fn sync_pull_blocking(
        &self,
        cursor: u64,
        limit: i64,
        device_id: &str,
    ) -> Result<SyncPullResponse, ApiError> {
        runtime().block_on(self.sync_pull(cursor, limit, device_id))
    }

    pub fn sync_push_blocking(
        &self,
        device_id: &str,
        items: Vec<SyncPushItem>,
    ) -> Result<SyncPushResponse, ApiError> {
        runtime().block_on(self.sync_push(device_id, items))
    }
}

fn urlencoding_lite(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct WireApiError {
    code: String,
    message: String,
    current_etag: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncPullResponse {
    pub protocol_version: u32,
    pub sync_epoch: u64,
    pub cursor: u64,
    pub changes: Vec<SyncChange>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncChange {
    pub id: u64,
    pub entity_type: String,
    pub entity_id: String,
    pub operation: String,
    pub payload_kind: String,
    pub payload_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncTodoPush {
    pub todo: Todo,
    pub base_revision: Option<u64>,
    pub if_match_etag: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPushItem {
    pub client_op_id: String,
    pub op: String,
    pub todo: Option<SyncTodoPush>,
    pub depends_on_client_op_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncPushResponse {
    pub results: Vec<SyncPushItemResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncPushItemResult {
    pub client_op_id: String,
    pub ok: bool,
    pub error_code: Option<String>,
    pub todo: Option<Todo>,
}

#[derive(Debug, Deserialize)]
pub struct LoginResponse {
    pub session_token: String,
    pub workspace_id: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiTokenInfo {
    pub id: String,
    pub name: String,
    pub scopes: Vec<String>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
struct NotebookListResponse {
    items: Vec<Notebook>,
}

#[derive(Debug, Deserialize)]
struct BatchMoveResponse {
    moved: u64,
}

#[derive(Debug, Deserialize)]
struct MemoListResponse {
    items: Vec<MemoSummary>,
}

#[derive(Debug, Deserialize)]
struct TodoListResponse {
    items: Vec<Todo>,
}

#[derive(Debug, Deserialize)]
struct TagSummary {
    tag: String,
}

#[derive(Debug, Deserialize)]
struct TagListResponse {
    items: Vec<TagSummary>,
}

#[derive(Debug, Deserialize)]
struct MemoOpenResponse {
    memo: Option<MemoDetailWire>,
    etag: String,
    lease: Option<EditLeaseWire>,
}

#[derive(Debug, Deserialize)]
struct MemoDetailWire {
    summary: Option<MemoSummary>,
    content: Option<MemoContentWire>,
}

#[derive(Debug, Deserialize)]
struct MemoContentWire {
    body: Option<MemoContentBodyWire>,
}

#[derive(Debug, Deserialize)]
enum MemoContentBodyWire {
    Markdown(String),
    ContentRef(String),
}

#[derive(Debug, Deserialize)]
struct EditLeaseWire {
    id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MemoSaveAck {
    pub protocol_version: u32,
    pub revision: u64,
    pub content_hash: String,
    pub etag: String,
    pub lease_id: Option<String>,
    pub saved_at: String,
}

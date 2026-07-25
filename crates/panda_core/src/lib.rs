//! Domain models aligned with panda-server Memo / Notebook wire types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncState {
    #[default]
    Synced,
    LocalCreated,
    LocalModified,
    LocalDeleted,
    Syncing,
    Conflict,
    Failed,
}

impl SyncState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synced => "synced",
            Self::LocalCreated => "local_created",
            Self::LocalModified => "local_modified",
            Self::LocalDeleted => "local_deleted",
            Self::Syncing => "syncing",
            Self::Conflict => "conflict",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "local_created" => Self::LocalCreated,
            "local_modified" => Self::LocalModified,
            "local_deleted" => Self::LocalDeleted,
            "syncing" => Self::Syncing,
            "conflict" => Self::Conflict,
            "failed" => Self::Failed,
            _ => Self::Synced,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Notebook {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub slug: Option<String>,
    pub path: String,
    pub depth: i32,
    pub sort_order: i32,
    pub memo_count: i64,
    pub is_deleted: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoSummary {
    pub id: String,
    pub notebook_id: String,
    pub title: Option<String>,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub is_pinned: bool,
    pub is_archived: bool,
    pub is_deleted: bool,
    pub revision: u64,
    pub content_hash: String,
    pub etag: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl MemoSummary {
    pub fn display_title(&self) -> &str {
        self.title
            .as_deref()
            .filter(|t| !t.is_empty())
            .unwrap_or("Untitled")
    }
}

/// Rough plain-text excerpt for the memo list (aligned with panda-server).
pub fn derive_excerpt(markdown: &str, max_chars: usize) -> String {
    let mut plain = String::with_capacity(markdown.len());
    let mut in_fence = false;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            plain.push_str(line);
            plain.push(' ');
            continue;
        }
        let mut l = line.trim();
        for prefix in [
            "# ", "## ", "### ", "#### ", "##### ", "###### ", "> ", "- ", "* ",
        ] {
            if let Some(rest) = l.strip_prefix(prefix) {
                l = rest;
                break;
            }
        }
        plain.push_str(l);
        plain.push(' ');
    }
    let collapsed: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    collapsed.chars().take(max_chars).collect::<String>() + "..."
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveMemo {
    pub id: String,
    pub notebook_id: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub markdown: String,
    pub etag: String,
    pub revision: u64,
    pub content_hash: String,
    pub lease_id: Option<String>,
    pub sync_state: SyncState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavFilter {
    All,
    Pinned,
    Trash,
    Notebook(String),
    Tag(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    #[default]
    Inbox,
    Open,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub note: String,
    pub status: TodoStatus,
    pub due_date: Option<String>,
    pub priority: i64,
    pub linked_memo_id: Option<String>,
    pub is_deleted: bool,
    pub revision: u64,
    pub etag: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TodoFilter {
    All,
    #[default]
    Inbox,
    Today,
    Upcoming,
    Completed,
    Trash,
}
impl TodoFilter {
    pub fn as_api_value(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Inbox => "inbox",
            Self::Today => "today",
            Self::Upcoming => "upcoming",
            Self::Completed => "completed",
            Self::Trash => "trash",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TodoOperationKind {
    Create,
    Update,
    Complete,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TodoOperation {
    pub operation_id: String,
    pub todo_id: String,
    pub kind: TodoOperationKind,
    pub payload_json: String,
    pub base_revision: Option<u64>,
    pub if_match_etag: Option<String>,
}

impl Default for NavFilter {
    fn default() -> Self {
        Self::All
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub api_url: String,
    pub token: String,
    pub device_id: String,
}

/// Normalize API base URL to a known Panda API root without duplicating path segments.
pub fn normalize_api_url(raw: &str) -> String {
    const DEFAULT_API_URL: &str = "http://127.0.0.1:8787/api/v1";
    const KNOWN_SUFFIXES: &[&str] = &["/panda/api/v1", "/panda/v1", "/api/v1"];

    let mut trimmed = raw.trim().trim_end_matches('/').to_string();
    // Repair legacy double-append: .../panda/api/v1/panda/v1
    while trimmed.ends_with("/panda/api/v1/panda/v1") {
        trimmed.truncate(trimmed.len() - "/panda/v1".len());
    }
    if trimmed.is_empty() {
        return DEFAULT_API_URL.into();
    }
    if KNOWN_SUFFIXES
        .iter()
        .any(|suffix| trimmed.ends_with(suffix))
    {
        return trimmed;
    }
    if trimmed.ends_with("/panda") {
        return format!("{trimmed}/api/v1");
    }
    format!("{trimmed}/api/v1")
}

#[cfg(test)]
mod tests {
    use super::normalize_api_url;

    #[test]
    fn keeps_panda_api_v1_url() {
        assert_eq!(
            normalize_api_url("https://notes.example/panda/api/v1"),
            "https://notes.example/panda/api/v1"
        );
    }

    #[test]
    fn keeps_api_v1_url() {
        assert_eq!(
            normalize_api_url("http://127.0.0.1:8787/api/v1/"),
            "http://127.0.0.1:8787/api/v1"
        );
    }

    #[test]
    fn appends_api_v1_to_host() {
        assert_eq!(
            normalize_api_url("http://127.0.0.1:8787"),
            "http://127.0.0.1:8787/api/v1"
        );
    }

    #[test]
    fn repairs_double_appended_panda_v1() {
        assert_eq!(
            normalize_api_url("https://notes.example/panda/api/v1/panda/v1"),
            "https://notes.example/panda/api/v1"
        );
    }
}

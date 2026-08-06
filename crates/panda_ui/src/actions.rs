use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use editor::Editor;
use gpui::{Context, Entity, Focusable, SharedString, Window, prelude::*, px};
use language::{Buffer, language_settings::SoftWrap};
use panda_core::{
    MemoSummary, NavFilter, SyncState, Todo, TodoFilter, TodoOperation, TodoOperationKind,
    TodoStatus,
};
use panda_session::app_data_dir;
use panda_sync::SyncEngine;
use settings::Settings;
use uuid::Uuid;
use workspace::ToolbarItemView;

use crate::shell::AppShell;
use crate::state::{MainState, MemoDisplayMode, Mode, SetupState, WorkspaceMode};
use crate::widgets::{display_title, field_editor};

fn queue_todo_operation(
    main: &MainState,
    todo: &Todo,
    kind: TodoOperationKind,
    base_revision: Option<u64>,
    if_match_etag: Option<String>,
) {
    let depends_on_write_id = main
        .store
        .pending_todo_operations()
        .ok()
        .and_then(|ops| ops.into_iter().rev().find(|op| op.todo_id == todo.id))
        .map(|op| {
            if op.write_id.is_empty() {
                op.operation_id
            } else {
                op.write_id
            }
        });
    let operation_id = Uuid::new_v4().to_string();
    let operation = TodoOperation {
        operation_id: operation_id.clone(),
        todo_id: todo.id.clone(),
        kind,
        payload_json: serde_json::to_string(todo).unwrap_or_default(),
        base_revision,
        if_match_etag,
        write_id: operation_id,
        depends_on_write_id,
        attempts: 0,
        last_error: None,
    };
    let _ = main.store.enqueue_todo_operation(&operation);
}

fn apply_local_todo(main: &mut MainState, todo: Todo, state: SyncState) {
    let _ = main.store.upsert_todo(&todo, state);
    if let Some(slot) = main.todos.iter_mut().find(|(item, _)| item.id == todo.id) {
        *slot = (todo, state);
    } else {
        main.todos.insert(0, (todo, state));
    }
}

impl AppShell {
    pub(crate) fn select_workspace(&mut self, mode: WorkspaceMode, cx: &mut Context<Self>) {
        if let Mode::Main(main) = &mut self.mode {
            main.workspace_mode = mode;
        }
        cx.notify();
    }

    pub(crate) fn select_todo_filter(&mut self, filter: TodoFilter, cx: &mut Context<Self>) {
        if let Mode::Main(main) = &mut self.mode {
            main.todo_filter = filter;
        }
        cx.notify();
    }

    pub(crate) fn visible_todos(&self) -> Vec<(Todo, SyncState)> {
        let Mode::Main(main) = &self.mode else {
            return Vec::new();
        };
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut seen_todo_ids = HashSet::new();
        main.todos
            .iter()
            .filter(|(todo, _)| {
                if main.todo_filter == TodoFilter::Trash {
                    todo.is_deleted
                } else {
                    !todo.is_deleted
                }
            })
            .filter(|(todo, _)| match main.todo_filter {
                TodoFilter::All => true,
                TodoFilter::Inbox => {
                    todo.status != TodoStatus::Completed && todo.due_date.is_none()
                }
                TodoFilter::Today => {
                    todo.status != TodoStatus::Completed
                        && todo.due_date.as_deref() == Some(today.as_str())
                }
                TodoFilter::Upcoming => {
                    todo.status != TodoStatus::Completed
                        && todo
                            .due_date
                            .as_deref()
                            .is_some_and(|due| due > today.as_str())
                }
                TodoFilter::Completed => todo.status == TodoStatus::Completed,
                TodoFilter::Trash => true,
            })
            .filter(|(todo, _)| seen_todo_ids.insert(todo.id.clone()))
            .cloned()
            .collect()
    }

    pub(crate) fn create_todo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if main.todo_create_in_flight {
            return;
        }
        main.todo_create_in_flight = true;
        let now = chrono::Utc::now().to_rfc3339();
        let todo = Todo {
            id: Uuid::new_v4().to_string(),
            title: "New task".into(),
            note: String::new(),
            status: TodoStatus::Inbox,
            due_date: None,
            priority: 0,
            linked_memo_id: None,
            is_deleted: false,
            revision: 0,
            etag: String::new(),
            created_at: now.clone(),
            updated_at: now,
            completed_at: None,
            deleted_at: None,
        };
        let todo_id = todo.id.clone();
        queue_todo_operation(main, &todo, TodoOperationKind::Create, None, None);
        apply_local_todo(main, todo, SyncState::LocalCreated);
        main.todo_create_in_flight = false;
        main.status = "Task created locally — syncing".into();
        self.open_todo(&todo_id, window, cx);
        self.refresh_remote(window, cx);
        cx.notify();
    }

    pub(crate) fn toggle_todo(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(todo) = main
            .todos
            .iter()
            .find(|(item, _)| item.id == id)
            .map(|(item, _)| item.clone())
        else {
            return;
        };
        let completed = todo.status != TodoStatus::Completed;
        let mut next = todo.clone();
        next.status = if completed {
            TodoStatus::Completed
        } else {
            TodoStatus::Open
        };
        next.completed_at = completed.then(|| chrono::Utc::now().to_rfc3339());
        next.updated_at = chrono::Utc::now().to_rfc3339();
        queue_todo_operation(
            main,
            &next,
            TodoOperationKind::Complete,
            (todo.revision > 0).then_some(todo.revision),
            (!todo.etag.is_empty()).then_some(todo.etag.clone()),
        );
        apply_local_todo(main, next, SyncState::LocalModified);
        main.status = if completed {
            "Task completed locally — syncing"
        } else {
            "Task restored locally — syncing"
        }
        .into();
        self.refresh_remote(window, cx);
        cx.notify();
    }

    pub(crate) fn toggle_selected_todo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = match &self.mode {
            Mode::Main(main) => main.selected_todo_id.clone(),
            Mode::Setup(_) => None,
        };
        if let Some(id) = id {
            self.toggle_todo(id, window, cx);
        }
    }

    pub(crate) fn delete_selected_todo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let todo = match &self.mode {
            Mode::Main(main) => main
                .selected_todo_id
                .as_deref()
                .and_then(|id| main.todos.iter().find(|(todo, _)| todo.id == id))
                .map(|(todo, _)| todo.clone()),
            Mode::Setup(_) => None,
        };
        let Some(todo) = todo else {
            return;
        };
        if let Mode::Main(main) = &mut self.mode {
            let mut next = todo.clone();
            next.is_deleted = true;
            next.deleted_at = Some(chrono::Utc::now().to_rfc3339());
            next.updated_at = chrono::Utc::now().to_rfc3339();
            queue_todo_operation(
                main,
                &next,
                TodoOperationKind::Delete,
                (todo.revision > 0).then_some(todo.revision),
                (!todo.etag.is_empty()).then_some(todo.etag.clone()),
            );
            apply_local_todo(main, next, SyncState::LocalDeleted);
            main.selected_todo_id = None;
            main.todo_title_editor = None;
            main.todo_note_editor = None;
            main.todo_due_date_editor = None;
            main.todo_priority_editor = None;
            main.status = "Task moved to trash locally — syncing".into();
        }
        self.refresh_remote(window, cx);
        cx.notify();
    }

    pub(crate) fn restore_selected_todo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let todo = match &self.mode {
            Mode::Main(main) => main
                .selected_todo_id
                .as_deref()
                .and_then(|id| main.todos.iter().find(|(todo, _)| todo.id == id))
                .map(|(todo, _)| todo.clone()),
            Mode::Setup(_) => None,
        };
        let Some(todo) = todo else {
            return;
        };
        if let Mode::Main(main) = &mut self.mode {
            let mut next = todo.clone();
            next.is_deleted = false;
            next.deleted_at = None;
            next.status = TodoStatus::Open;
            next.updated_at = chrono::Utc::now().to_rfc3339();
            queue_todo_operation(
                main,
                &next,
                TodoOperationKind::Restore,
                (todo.revision > 0).then_some(todo.revision),
                (!todo.etag.is_empty()).then_some(todo.etag.clone()),
            );
            apply_local_todo(main, next, SyncState::LocalModified);
            main.todo_filter = TodoFilter::Inbox;
            main.status = "Task restored locally — syncing".into();
        }
        self.refresh_remote(window, cx);
        cx.notify();
    }

    pub(crate) fn permanently_delete_selected_todo(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let todo = match &self.mode {
            Mode::Main(main) => main
                .selected_todo_id
                .as_deref()
                .and_then(|id| main.todos.iter().find(|(todo, _)| todo.id == id))
                .map(|(todo, _)| todo.clone()),
            Mode::Setup(_) => None,
        };
        let Some(todo) = todo else {
            return;
        };
        if !todo.is_deleted {
            return;
        }
        let client = match &self.mode {
            Mode::Main(main) => main.client.clone(),
            Mode::Setup(_) => return,
        };
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || client.delete_todo_blocking(&todo, true)).await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(()) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.selected_todo_id = None;
                            main.todo_title_editor = None;
                            main.todo_note_editor = None;
                            main.todo_due_date_editor = None;
                            main.todo_priority_editor = None;
                            main.status = "Task permanently deleted".into();
                        }
                        this.refresh_remote(window, cx);
                    }
                    Err(error) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(error.to_string().into());
                            main.status = "Permanent delete task failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_todo(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let todo = match &self.mode {
            Mode::Main(main) => main
                .todos
                .iter()
                .find(|(todo, _)| todo.id == id)
                .map(|(todo, _)| todo.clone()),
            Mode::Setup(_) => None,
        };
        let Some(todo) = todo else {
            return;
        };
        let title_editor = field_editor(window, cx, &todo.title);
        let note = todo.note.clone();
        let languages = self.languages.clone();
        let buffer = cx.new(|cx| {
            let buffer = Buffer::local(note, cx);
            buffer.set_language_registry(languages);
            buffer
        });
        let note_editor = cx.new(|cx| {
            let mut editor = Editor::for_buffer(buffer, None, window, cx);
            editor.set_soft_wrap_mode(SoftWrap::EditorWidth, cx);
            let gutter = editor::EditorSettings::get_global(cx).gutter;
            editor.set_show_gutter(gutter.line_numbers, cx);
            editor.set_show_line_numbers(gutter.line_numbers, cx);
            editor.set_show_git_diff_gutter(false, cx);
            editor.set_show_code_actions(false, cx);
            editor.set_show_breakpoints(false, cx);
            editor.set_show_runnables(false, cx);
            editor.set_show_bookmarks(false, cx);
            editor.set_use_modal_editing(true);
            editor
        });
        let due_date_editor =
            field_editor(window, cx, todo.due_date.as_deref().unwrap_or_default());
        let priority_editor = field_editor(window, cx, &todo.priority.to_string());
        if let Mode::Main(main) = &mut self.mode {
            main.selected_todo_id = Some(todo.id);
            main.todo_title_editor = Some(title_editor);
            main.todo_note_editor = Some(note_editor);
            main.todo_due_date_editor = Some(due_date_editor);
            main.todo_priority_editor = Some(priority_editor);
        }
        cx.notify();
    }

    pub(crate) fn save_todo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(id) = main.selected_todo_id.clone() else {
            return;
        };
        let Some((existing, _)) = main.todos.iter().find(|(todo, _)| todo.id == id) else {
            return;
        };
        let mut todo = existing.clone();
        if let Some(editor) = &main.todo_title_editor {
            todo.title = editor.read(cx).text(cx);
        }
        if let Some(editor) = &main.todo_note_editor {
            todo.note = editor.read(cx).text(cx);
        }
        if let Some(editor) = &main.todo_due_date_editor {
            let due_date = editor.read(cx).text(cx).trim().to_string();
            todo.due_date = (!due_date.is_empty()).then_some(due_date);
        }
        if let Some(editor) = &main.todo_priority_editor {
            if let Ok(priority) = editor.read(cx).text(cx).trim().parse::<i64>() {
                todo.priority = priority.clamp(0, 3);
            }
        }
        let original_revision = todo.revision;
        let original_etag = todo.etag.clone();
        let has_pending_create = main
            .store
            .pending_todo_operations()
            .ok()
            .is_some_and(|ops| {
                ops.iter()
                    .any(|op| op.todo_id == todo.id && op.kind == TodoOperationKind::Create)
            });
        let kind = if original_revision == 0 && !has_pending_create {
            TodoOperationKind::Create
        } else {
            TodoOperationKind::Update
        };
        queue_todo_operation(
            main,
            &todo,
            kind,
            (original_revision > 0).then_some(original_revision),
            (!original_etag.is_empty()).then_some(original_etag),
        );
        main.todo_save_in_flight = false;
        apply_local_todo(main, todo, SyncState::LocalModified);
        main.status = "Task saved locally — syncing".into();
        self.refresh_remote(window, cx);
        cx.notify();
    }

    pub(crate) fn set_todo_due_date(
        &mut self,
        due_date: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = (match &self.mode {
            Mode::Main(main) => main.todo_due_date_editor.clone(),
            Mode::Setup(_) => None,
        }) else {
            return;
        };
        editor.update(cx, |editor, cx| {
            editor.set_text(due_date.unwrap_or_default(), window, cx);
        });
        cx.notify();
    }

    pub(crate) fn set_todo_priority(
        &mut self,
        priority: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = (match &self.mode {
            Mode::Main(main) => main.todo_priority_editor.clone(),
            Mode::Setup(_) => None,
        }) else {
            return;
        };
        editor.update(cx, |editor, cx| {
            editor.set_text(priority.to_string(), window, cx);
        });
        cx.notify();
    }
    pub(crate) fn blank_setup(window: &mut Window, cx: &mut Context<Self>) -> SetupState {
        SetupState {
            name: field_editor(window, cx, "Local"),
            api_url: field_editor(window, cx, "http://127.0.0.1:8787/api/v1"),
            username: field_editor(window, cx, "admin"),
            password: field_editor(window, cx, "admin123"),
            token: field_editor(window, cx, ""),
            error: None,
            busy: false,
        }
    }

    pub(crate) fn enter_main(
        &mut self,
        instance: panda_core::Instance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let db_path = app_data_dir()
            .map(|d| d.join(format!("cache-{}.sqlite", instance.id)))
            .unwrap_or_else(|_| std::env::temp_dir().join("panda.sqlite"));
        let store = match panda_store::LocalStore::open(&db_path) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                let msg: SharedString = format!("open store: {e}").into();
                match &mut self.mode {
                    Mode::Setup(setup) => setup.error = Some(msg),
                    Mode::Main(main) => main.error = Some(msg),
                }
                return;
            }
        };
        let _ = self.session.set_active(&instance.id);
        let _ = store.set_meta("active_instance", &instance.id);
        let client = panda_api::ApiClient::new(instance.api_url.clone(), instance.token.clone());
        let memo_display_mode = match &self.mode {
            Mode::Main(main) => main.memo_display_mode,
            _ => Default::default(),
        };
        let (nav_width, list_width) = MainState::load_pane_widths(&store);
        let nav_collapsed = MainState::load_nav_collapsed(&store);
        self.mode = Mode::Main(MainState {
            instance,
            client,
            store,
            notebooks: Vec::new(),
            memos: Vec::new(),
            workspace_mode: WorkspaceMode::Memos,
            todo_filter: TodoFilter::Inbox,
            todos: Vec::new(),
            selected_todo_id: None,
            todo_title_editor: None,
            todo_note_editor: None,
            todo_due_date_editor: None,
            todo_priority_editor: None,
            todo_create_in_flight: false,
            todo_save_in_flight: false,
            filter: NavFilter::All,
            selected_id: None,
            active: None,
            editor: None,
            title_editor: None,
            tag_editor: None,
            search_editor: None,
            buffer_search_bar: None,
            search_query: String::new(),
            search_results: None,
            available_tags: Vec::new(),
            preview: None,
            preview_scroll_handle: gpui::ScrollHandle::new(),
            memo_display_mode,
            status: "Loading...".into(),
            error: None,
            save_generation: 0,
            draft_generation: 0,
            preview_generation: 0,
            save_in_flight: false,
            save_pending: false,
            nav_width,
            list_width,
            nav_collapsed,
            collapsed_notebooks: Default::default(),
            context_menu: None,
            rename_dialog: None,
            _rename_sub: None,
            _title_sub: None,
            _tag_sub: None,
            _search_sub: None,
            _buffer_search_sub: None,
            _buffer_sub: None,
            _preview_sub: None,
        });
        if let Mode::Main(main) = &mut self.mode {
            let languages = self.languages.clone();
            let buffer_search_bar = cx.new(|cx| {
                let mut bar = search::BufferSearchBar::new(Some(languages), window, cx);
                bar.set_input_width_override(Some(px(280.)), cx);
                bar
            });
            search::buffer_search::set_standalone_buffer_search_bar(Some(&buffer_search_bar), cx);
            main._buffer_search_sub = Some(cx.subscribe(
                &buffer_search_bar,
                |_, _, _: &search::buffer_search::Event, cx| {
                    cx.notify();
                },
            ));
            main.buffer_search_bar = Some(buffer_search_bar);

            let search_editor = cx.new(|cx| {
                let mut editor = Editor::single_line(window, cx);
                editor.set_placeholder_text("Search memos...", window, cx);
                editor.set_show_gutter(false, cx);
                editor.set_use_modal_editing(false);
                editor
            });
            main._search_sub = Some(cx.subscribe(&search_editor, |this, editor, event, cx| {
                if matches!(
                    event,
                    editor::EditorEvent::BufferEdited | editor::EditorEvent::Edited { .. }
                ) {
                    this.update_search_query(editor.read(cx).text(cx), cx);
                }
            }));
            main.search_editor = Some(search_editor);
            let tag_editor = cx.new(|cx| {
                let mut editor = Editor::single_line(window, cx);
                editor.set_placeholder_text("Add tag...", window, cx);
                editor.set_show_gutter(false, cx);
                editor.set_use_modal_editing(false);
                editor
            });
            main._tag_sub = Some(cx.subscribe(
                &tag_editor,
                |this, editor, event, cx| match event {
                    editor::EditorEvent::Blurred => {
                        let text = editor.read(cx).text(cx);
                        let _ = this.add_tag_to_active(text, cx);
                        cx.spawn(async move |this, cx| {
                            this.update_in(cx, |shell, window, cx| {
                                shell.clear_tag_editor(window, cx);
                            })
                            .ok();
                        })
                        .detach();
                    }
                    editor::EditorEvent::BufferEdited | editor::EditorEvent::Edited { .. } => {
                        cx.notify();
                    }
                    _ => {}
                },
            ));
            main.tag_editor = Some(tag_editor);
        }
        self.refresh_remote(window, cx);
        // Quiet background sync: refresh list and reload editor when remote moves ahead
        // and the open memo is still Synced (won't fork local drafts on a timer).
        cx.spawn_in(window, async move |this, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(8)).await;
                let keep_going = this
                    .update_in(cx, |this, window, cx| {
                        if !matches!(this.mode, Mode::Main(_)) {
                            return false;
                        }
                        if let Mode::Main(main) = &this.mode {
                            if main.save_in_flight {
                                return true;
                            }
                        }
                        this.refresh_remote_inner(window, cx, false);
                        true
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn refresh_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_remote_inner(window, cx, true);
    }

    fn refresh_remote_inner(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        reconcile_dirty: bool,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if reconcile_dirty {
            main.status = "Refreshing...".into();
        }
        main.error = None;
        let client = main.client.clone();
        let store = main.store.clone();
        let engine = SyncEngine::new(
            client.clone(),
            store.clone(),
            main.instance.device_id.clone(),
        );
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                // Replay the durable Todo outbox and pull the cursor before the
                // regular REST refresh. REST remains the authoritative full
                // snapshot for the list, while the sync journal carries edits
                // made on other devices and conflict state.
                let _sync_report = engine.sync_todos();
                let notebooks = client.list_notebooks_blocking()?;
                let tags = client.list_tags_blocking().unwrap_or_default();
                let active = client.list_memos_blocking(None, false, None, 200)?;
                let trashed = client.list_memos_blocking(None, true, None, 200)?;
                let mut memos = active;
                memos.extend(trashed);
                let mut todos = client.list_todos_blocking("all", 200)?;
                todos.extend(client.list_todos_blocking("trash", 200)?);
                Ok::<_, panda_api::ApiError>((notebooks, memos, tags, todos))
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                let Mode::Main(main) = &mut this.mode else {
                    return;
                };
                let mut reload_active: Option<(String, bool)> = None;
                match result {
                    Ok((notebooks, memos, tags, todos)) => {
                        for summary in &memos {
                            let _ = main.store.upsert_summary(summary, SyncState::Synced);
                        }
                        main.notebooks = notebooks;
                        main.available_tags = tags;
                        main.memos = memos.into_iter().map(|s| (s, SyncState::Synced)).collect();
                        let local_todos = store.list_todos().unwrap_or_default();
                        let mut local_by_id = local_todos
                            .into_iter()
                            .map(|(todo, state)| (todo.id.clone(), (todo, state)))
                            .collect::<std::collections::HashMap<_, _>>();
                        let mut merged_todos = Vec::with_capacity(todos.len() + local_by_id.len());
                        for remote in todos {
                            let id = remote.id.clone();
                            if let Some((local, state)) = local_by_id.remove(&id) {
                                if matches!(
                                    state,
                                    SyncState::LocalModified
                                        | SyncState::LocalCreated
                                        | SyncState::LocalDeleted
                                        | SyncState::Failed
                                        | SyncState::Conflict
                                ) {
                                    merged_todos.push((local, state));
                                } else {
                                    let _ = main.store.upsert_todo(&remote, SyncState::Synced);
                                    merged_todos.push((remote, SyncState::Synced));
                                }
                            } else {
                                let _ = main.store.upsert_todo(&remote, SyncState::Synced);
                                merged_todos.push((remote, SyncState::Synced));
                            }
                        }
                        // Keep offline-created and locally deleted records in
                        // memory even when the server snapshot does not have
                        // them yet (or hides them from the active endpoint).
                        merged_todos.extend(local_by_id.into_values());
                        main.todos = merged_todos;
                        if let Ok(local) = store.list_summaries() {
                            for (summary, state) in local {
                                if matches!(
                                    state,
                                    SyncState::LocalModified
                                        | SyncState::LocalCreated
                                        | SyncState::Failed
                                        | SyncState::Conflict
                                ) {
                                    if let Some(slot) =
                                        main.memos.iter_mut().find(|(m, _)| m.id == summary.id)
                                    {
                                        slot.1 = state;
                                    }
                                }
                            }
                        }
                        if let Some(active) = main.active.as_ref() {
                            if let Some((summary, _)) =
                                main.memos.iter().find(|(m, _)| m.id == active.id)
                            {
                                let remote_newer = summary.revision > active.revision
                                    || summary.etag != active.etag
                                    || summary.content_hash != active.content_hash;
                                if remote_newer {
                                    let dirty = matches!(
                                        active.sync_state,
                                        SyncState::LocalModified | SyncState::LocalCreated
                                    );
                                    if dirty && !reconcile_dirty {
                                        main.status =
                                            "Remote updated while editing — refresh to reconcile"
                                                .into();
                                    } else {
                                        reload_active = Some((active.id.clone(), dirty));
                                    }
                                }
                            }
                        }
                        if reload_active.is_none() && reconcile_dirty {
                            main.status = format!(
                                "{} | {} notebooks | {} memos",
                                main.instance.name,
                                main.notebooks.len(),
                                main.memos.len()
                            )
                            .into();
                        }
                    }
                    Err(e) => {
                        if reconcile_dirty {
                            main.error = Some(e.to_string().into());
                            main.status = "Refresh failed".into();
                        }
                    }
                }
                if let Some((id, dirty)) = reload_active {
                    if dirty {
                        let markdown = main
                            .editor
                            .as_ref()
                            .map(|e| e.read(cx).text(cx))
                            .or_else(|| main.active.as_ref().map(|a| a.markdown.clone()))
                            .unwrap_or_default();
                        this.preserve_local_as_conflict_copy(&markdown, cx);
                    }
                    this.open_memo_prefer_remote(&id, window, cx);
                    if let Mode::Main(main) = &mut this.mode {
                        main.status = if dirty {
                            "Remote updated — local edits saved as conflict copy".into()
                        } else {
                            "Remote updated — reloading editor".into()
                        };
                    }
                    return;
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn visible_memos(&self) -> Vec<(panda_core::MemoSummary, SyncState)> {
        let Mode::Main(main) = &self.mode else {
            return Vec::new();
        };
        let mut memos = main
            .memos
            .iter()
            .filter(|(m, _)| match &main.filter {
                NavFilter::All => !m.is_deleted,
                NavFilter::Pinned => m.is_pinned && !m.is_deleted,
                NavFilter::Trash => m.is_deleted,
                NavFilter::Notebook(id) => !m.is_deleted && m.notebook_id == *id,
                NavFilter::Tag(tag) => {
                    !m.is_deleted
                        && m.tags
                            .iter()
                            .any(|memo_tag| memo_tag.eq_ignore_ascii_case(tag))
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        memos.sort_by(|(left, _), (right, _)| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        memos
    }

    pub(crate) fn connect_login(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Setup(setup) = &mut self.mode else {
            return;
        };
        setup.busy = true;
        setup.error = None;
        let name = setup.name.read(cx).text(cx);
        let api_url = setup.api_url.read(cx).text(cx);
        let username = setup.username.read(cx).text(cx);
        let password = setup.password.read(cx).text(cx);
        cx.notify();

        match self.session.add_via_login(
            name.trim(),
            api_url.trim(),
            username.trim(),
            password.trim(),
        ) {
            Ok(instance) => self.enter_main(instance, window, cx),
            Err(e) => {
                if let Mode::Setup(setup) = &mut self.mode {
                    setup.busy = false;
                    setup.error = Some(e.to_string().into());
                }
                cx.notify();
            }
        }
    }

    pub(crate) fn connect_token(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Setup(setup) = &mut self.mode else {
            return;
        };
        let name = setup.name.read(cx).text(cx);
        let api_url = setup.api_url.read(cx).text(cx);
        let token = setup.token.read(cx).text(cx);
        if token.trim().is_empty() {
            setup.error = Some("Token is empty".into());
            cx.notify();
            return;
        }
        match self
            .session
            .add_via_token(name.trim(), api_url.trim(), token.trim())
        {
            Ok(instance) => self.enter_main(instance, window, cx),
            Err(e) => {
                setup.error = Some(e.to_string().into());
                cx.notify();
            }
        }
    }

    pub(crate) fn open_memo(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_memo_inner(id, false, window, cx);
    }

    /// Reload memo from server, ignoring any local draft for this id.
    pub(crate) fn open_memo_prefer_remote(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_memo_inner(id, true, window, cx);
    }

    fn open_memo_inner(
        &mut self,
        id: &str,
        prefer_remote: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        // Persist outgoing draft only when switching away from another memo.
        // Prefer-remote reload of the same memo must not snapshot stale buffer as LocalModified.
        if let (Some(active), Some(editor)) = (&main.active, &main.editor) {
            if active.id != id {
                let text = editor.read(cx).text(cx);
                let _ = main
                    .store
                    .save_draft(&active.id, &text, SyncState::LocalModified);
            }
        }

        main.status = "Opening...".into();
        main.error = None;
        let client = main.client.clone();
        let store = main.store.clone();
        let memo_id = id.to_string();
        let languages = self.languages.clone();

        cx.spawn_in(window, async move |this, cx| {
            let opened = smol::unblock({
                let memo_id = memo_id.clone();
                move || client.open_memo_blocking(&memo_id)
            })
            .await;

            this.update_in(cx, |this, window, cx| {
                let Mode::Main(main) = &mut this.mode else {
                    return;
                };
                match opened {
                    Ok(mut active) => {
                        if prefer_remote {
                            let _ = store.mark_synced(
                                &active.id,
                                &active.etag,
                                active.revision,
                                &active.content_hash,
                                active.lease_id.as_deref(),
                            );
                            active.sync_state = SyncState::Synced;
                        } else if let Ok(Some(local)) = store.get_active(&active.id) {
                            if matches!(
                                local.sync_state,
                                SyncState::LocalModified | SyncState::Failed
                            ) {
                                active.markdown = local.markdown;
                                active.sync_state = local.sync_state;
                            }
                        }
                        let _ = store.upsert_active(&active);
                        main.selected_id = Some(active.id.clone());
                        main.status = format!(
                            "{} | {}",
                            display_title(&active),
                            active.sync_state.as_str()
                        )
                        .into();

                        let markdown = active.markdown.clone();
                        let buffer = cx.new(|cx| {
                            let buffer = Buffer::local(markdown, cx);
                            buffer.set_language_registry(languages.clone());
                            buffer
                        });
                        let editor = cx.new(|cx| {
                            let gutter = editor::EditorSettings::get_global(cx).gutter;
                            let mut editor = Editor::for_buffer(buffer.clone(), None, window, cx);
                            editor.set_soft_wrap_mode(SoftWrap::EditorWidth, cx);
                            editor.set_use_modal_editing(true);
                            editor.set_show_gutter(gutter.line_numbers, cx);
                            editor.set_show_line_numbers(gutter.line_numbers, cx);
                            editor.set_show_git_diff_gutter(false, cx);
                            editor.set_show_code_actions(false, cx);
                            editor.set_show_breakpoints(gutter.breakpoints, cx);
                            editor.set_show_runnables(false, cx);
                            editor.set_show_bookmarks(false, cx);
                            editor.set_custom_context_menu(|editor, _point, window, cx| {
                                let has_selection =
                                    editor.has_non_empty_selection(&editor.display_snapshot(cx));
                                Some(ui::ContextMenu::build(window, cx, |menu, _, _| {
                                    menu.action_disabled_when(
                                        !has_selection,
                                        "Cut",
                                        Box::new(editor::actions::Cut),
                                    )
                                    .action_disabled_when(
                                        !has_selection,
                                        "Copy",
                                        Box::new(editor::actions::Copy),
                                    )
                                    .action("Paste", Box::new(editor::actions::Paste))
                                    .separator()
                                    .action("Select All", Box::new(editor::actions::SelectAll))
                                }))
                            });
                            editor
                        });

                        let memo_id_for_sub = active.id.clone();
                        let preview = cx.new(|cx| {
                            markdown::Markdown::new(
                                SharedString::from(active.markdown.clone()),
                                Some(languages.clone()),
                                None,
                                cx,
                            )
                        });
                        main._buffer_sub =
                            Some(cx.subscribe(&buffer, move |this, _, event, cx| {
                                if let language::BufferEvent::Edited { .. } = event {
                                    this.schedule_save(&memo_id_for_sub, cx);
                                    this.schedule_preview_update(false, cx);
                                }
                            }));
                        main.preview_scroll_handle = gpui::ScrollHandle::new();
                        main._preview_sub = Some(cx.subscribe_in(
                            &editor,
                            window,
                            |this, editor, event: &editor::EditorEvent, window, cx| {
                                if matches!(event, editor::EditorEvent::SelectionsChanged { .. }) {
                                    this.sync_preview_to_editor_selection(editor, window, cx);
                                }
                            },
                        ));

                        let editor_weak = editor.downgrade();
                        let languages2 = languages.clone();
                        cx.spawn(async move |_this, cx| {
                            if let Ok(md) = languages2.language_for_name("Markdown").await {
                                editor_weak
                                    .update(cx, |editor, cx| {
                                        editor.buffer().update(cx, |mb, cx| {
                                            if let Some(buf) = mb.as_singleton() {
                                                buf.update(cx, |b, cx| {
                                                    b.set_language(Some(md), cx)
                                                });
                                            }
                                        });
                                    })
                                    .ok();
                            }
                        })
                        .detach();

                        let title_text = display_title(&active).to_string();
                        let title_editor = cx.new(|cx| {
                            let mut editor = Editor::single_line(window, cx);
                            editor.set_text(title_text, window, cx);
                            editor.set_show_gutter(false, cx);
                            editor.set_use_modal_editing(false);
                            editor
                        });
                        let memo_id_for_title = active.id.clone();
                        main._title_sub =
                            Some(cx.subscribe(&title_editor, move |this, editor, event, cx| {
                                if let editor::EditorEvent::Blurred = event {
                                    this.commit_title_from_editor(&memo_id_for_title, &editor, cx);
                                }
                            }));

                        main.active = Some(active);
                        main.editor = Some(editor.clone());
                        main.title_editor = Some(title_editor);
                        main.preview = Some(preview);
                        if let Some(bar) = main.buffer_search_bar.clone() {
                            bar.update(cx, |bar, cx| {
                                bar.set_active_pane_item(Some(&editor), window, cx);
                            });
                        }
                    }
                    Err(e) => {
                        main.error = Some(e.to_string().into());
                        main.status = "Open failed".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn schedule_save(&mut self, memo_id: &str, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(editor) = main.editor.clone() else {
            return;
        };
        let Some(active) = main.active.as_mut() else {
            return;
        };
        if active.id != memo_id {
            return;
        }

        let text = editor.read(cx).text(cx);
        active.markdown = text;
        let state_changed = active.sync_state != SyncState::LocalModified;
        active.sync_state = SyncState::LocalModified;
        if let Some((summary, state)) = main.memos.iter_mut().find(|(m, _)| m.id == memo_id) {
            if summary.title != active.title {
                summary.title = active.title.clone();
            }
            *state = SyncState::LocalModified;
        }
        main.status = "LocalModified".into();
        main.save_generation += 1;
        let save_gen = main.save_generation;
        main.draft_generation += 1;
        let draft_gen = main.draft_generation;
        let id = active.id.clone();
        let draft_store = main.store.clone();

        // A local crash-safe draft is important, but SQLite work on every key
        // stroke is visible on long notes. Persist only the settled buffer.
        cx.spawn({
            let id = id.clone();
            async move |this, cx| {
                smol::Timer::after(Duration::from_millis(300)).await;
                let draft = this
                    .read_with(cx, |this, cx| match &this.mode {
                        Mode::Main(main)
                            if main.draft_generation == draft_gen
                                && main.active.as_ref().is_some_and(|a| a.id == id) =>
                        {
                            main.editor.as_ref().map(|editor| editor.read(cx).text(cx))
                        }
                        _ => None,
                    })
                    .ok()
                    .flatten();
                if let Some(draft) = draft {
                    let _ = smol::unblock(move || {
                        draft_store.save_draft(&id, &draft, SyncState::LocalModified)
                    })
                    .await;
                }
            }
        })
        .detach();

        if main.save_in_flight {
            main.save_pending = true;
            if state_changed {
                cx.notify();
            }
            return;
        }

        cx.spawn(async move |this, cx| {
            smol::Timer::after(Duration::from_millis(500)).await;
            let still = this
                .read_with(cx, |this, _| match &this.mode {
                    Mode::Main(main) => main.save_generation == save_gen,
                    _ => false,
                })
                .unwrap_or(false);
            if !still {
                return;
            }
            this.update(cx, |this, cx| {
                this.flush_save(&id, false, cx);
            })
            .ok();
        })
        .detach();
        // The editor redraws its own buffer. Re-rendering the entire three-pane
        // shell for every keystroke is unnecessary; notify only when the sync
        // indicator actually changes.
        if state_changed {
            cx.notify();
        }
    }

    /// Keep the Live/Read Markdown projection scrolled to the editor cursor.
    /// Mirrors `markdown_preview`'s selection-driven sync: only autoscroll when
    /// the source editor is focused so manual preview scrolling isn't stolen.
    fn sync_preview_to_editor_selection(
        &mut self,
        editor: &Entity<Editor>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &self.mode else {
            return;
        };
        if !matches!(
            main.memo_display_mode,
            MemoDisplayMode::Live | MemoDisplayMode::Read
        ) {
            return;
        }
        let Some(preview) = main.preview.clone() else {
            return;
        };
        let (source_index, editor_focused) = editor.update(cx, |editor, cx| {
            let index = Self::editor_source_index(editor, cx);
            let focused = editor.focus_handle(cx).is_focused(window);
            (index, focused)
        });
        let Some(source_index) = source_index else {
            return;
        };
        preview.update(cx, |markdown, cx| {
            markdown.set_active_root_for_source_index(Some(source_index), cx);
            if editor_focused {
                markdown.request_autoscroll_to_source_index(source_index, cx);
            }
        });
    }

    fn editor_source_index(editor: &Editor, cx: &mut gpui::App) -> Option<usize> {
        let display_snapshot = editor.display_snapshot(cx);
        let source_offset = editor
            .selections
            .last::<editor::MultiBufferOffset>(&display_snapshot)
            .range()
            .start;
        let buffer = editor.buffer().read(cx).as_singleton()?;
        let buffer_id = buffer.read(cx).remote_id();
        let (buffer_snapshot, buffer_offset) = display_snapshot
            .buffer_snapshot()
            .point_to_buffer_offset(source_offset)?;
        if buffer_snapshot.remote_id() == buffer_id {
            Some(buffer_offset.0)
        } else {
            None
        }
    }

    /// Update the rendered projection only when it is actually visible. The
    /// Markdown component parses whole documents, so coalescing keystrokes here
    /// keeps the Vim/editor input path independent from preview cost.
    fn schedule_preview_update(&mut self, immediate: bool, cx: &mut Context<Self>) {
        let (generation, preview, editor) = {
            let Mode::Main(main) = &mut self.mode else {
                return;
            };
            if main.memo_display_mode == MemoDisplayMode::Source {
                return;
            }
            let Some(preview) = main.preview.clone() else {
                return;
            };
            main.preview_generation += 1;
            (main.preview_generation, preview, main.editor.clone())
        };
        cx.spawn(async move |this, cx| {
            if !immediate {
                smol::Timer::after(Duration::from_millis(180)).await;
            }
            let source = this
                .read_with(cx, |this, cx| match &this.mode {
                    Mode::Main(main)
                        if main.preview_generation == generation
                            && main.memo_display_mode != MemoDisplayMode::Source =>
                    {
                        main.editor.as_ref().map(|editor| editor.read(cx).text(cx))
                    }
                    _ => None,
                })
                .ok()
                .flatten();
            if let Some(source) = source {
                let source_index = editor.as_ref().and_then(|editor| {
                    editor.update(cx, |editor, cx| Self::editor_source_index(editor, cx))
                });
                let still_previewing = this
                    .read_with(cx, |this, _| match &this.mode {
                        Mode::Main(main) => matches!(
                            main.memo_display_mode,
                            MemoDisplayMode::Live | MemoDisplayMode::Read
                        ),
                        Mode::Setup(_) => false,
                    })
                    .unwrap_or(false);
                preview.update(cx, |markdown, cx| {
                    markdown.replace(SharedString::from(source), cx);
                    if still_previewing {
                        if let Some(source_index) = source_index {
                            markdown.set_active_root_for_source_index(Some(source_index), cx);
                            markdown.request_autoscroll_to_source_index(source_index, cx);
                        }
                    }
                });
            }
        })
        .detach();
    }

    /// Send one save using the latest local etag/markdown. Serializes via `save_in_flight`.
    pub(crate) fn flush_save(&mut self, memo_id: &str, is_retry: bool, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if main.save_in_flight && !is_retry {
            main.save_pending = true;
            return;
        }
        let Some(active) = main.active.as_ref() else {
            return;
        };
        if active.id != memo_id {
            return;
        }
        let markdown = main
            .editor
            .as_ref()
            .map(|e| e.read(cx).text(cx))
            .unwrap_or_else(|| active.markdown.clone());
        let client = main.client.clone();
        let store = main.store.clone();
        let id = active.id.clone();
        let etag = active.etag.clone();
        let revision = active.revision;
        let content_hash = active.content_hash.clone();
        let lease_id = active.lease_id.clone();
        let title = active.title.clone();
        let tags = active.tags.clone();
        main.save_in_flight = true;
        main.save_pending = false;

        cx.spawn(async move |this, cx| {
            let save = smol::unblock({
                let client = client.clone();
                let id = id.clone();
                let etag = etag.clone();
                let content_hash = content_hash.clone();
                let lease_id = lease_id.clone();
                let title = title.clone();
                let tags = tags.clone();
                let markdown = markdown.clone();
                move || {
                    client.save_memo_blocking(
                        &id,
                        &markdown,
                        &etag,
                        revision,
                        &content_hash,
                        lease_id.as_deref(),
                        title.as_deref(),
                        Some(&tags),
                    )
                }
            })
            .await;

            this.update(cx, |this, cx| {
                let Mode::Main(main) = &mut this.mode else {
                    return;
                };
                main.save_in_flight = false;

                match save {
                    Ok(ack) => {
                        this.apply_save_ack(&id, &ack, &markdown, &store, cx);
                        let Mode::Main(main) = &mut this.mode else {
                            return;
                        };
                        if main.save_pending
                            || main
                                .active
                                .as_ref()
                                .is_some_and(|a| a.id == id && a.sync_state != SyncState::Synced)
                        {
                            main.save_pending = false;
                            let id = id.clone();
                            this.flush_save(&id, false, cx);
                        }
                    }
                    Err(e) if e.is_lease_conflict() => {
                        main.status = "Lease expired — reopen memo".into();
                        main.error = Some(e.to_string().into());
                        if let Some(active) = main.active.as_mut() {
                            if active.id == id {
                                active.sync_state = SyncState::Failed;
                            }
                        }
                    }
                    Err(e) if e.is_revision_conflict() => {
                        // Never steal the server etag and overwrite remote with stale local text.
                        this.create_conflict_copy(&id, &markdown, &e, cx);
                        if let Mode::Main(main) = &mut this.mode {
                            if let Some(active) = main.active.as_mut() {
                                if active.id == id {
                                    active.sync_state = SyncState::Conflict;
                                }
                            }
                            main.status =
                                "Conflict — local copy created; refresh to load remote".into();
                        }
                    }
                    Err(e) if e.is_conflict() => {
                        this.create_conflict_copy(&id, &markdown, &e, cx);
                    }
                    Err(e) => {
                        let _ = store.save_draft(&id, &markdown, SyncState::Failed);
                        if let Some(active) = main.active.as_mut() {
                            if active.id == id {
                                active.sync_state = SyncState::Failed;
                            }
                        }
                        main.error = Some(e.to_string().into());
                        main.status = "Save failed".into();
                        if main.save_pending {
                            main.save_pending = false;
                            let id = id.clone();
                            this.flush_save(&id, false, cx);
                            return;
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn apply_save_ack(
        &mut self,
        id: &str,
        ack: &panda_api::MemoSaveAck,
        markdown: &str,
        store: &Arc<panda_store::LocalStore>,
        _cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let should_apply = main
            .active
            .as_ref()
            .is_some_and(|a| a.id == id && ack.revision >= a.revision);
        if !should_apply
            && !main
                .memos
                .iter()
                .any(|(m, _)| m.id == id && ack.revision >= m.revision)
        {
            return;
        }
        let _ = store.mark_synced(
            id,
            &ack.etag,
            ack.revision,
            &ack.content_hash,
            ack.lease_id.as_deref(),
        );
        if let Some(active) = main.active.as_mut() {
            if active.id == id && ack.revision >= active.revision {
                active.etag = ack.etag.clone();
                active.revision = ack.revision;
                active.content_hash = ack.content_hash.clone();
                active.lease_id = ack.lease_id.clone();
                active.sync_state = SyncState::Synced;
                active.markdown = markdown.to_string();
            }
        }
        if let Some((summary, state)) = main.memos.iter_mut().find(|(m, _)| m.id == id) {
            if ack.revision >= summary.revision {
                summary.etag = ack.etag.clone();
                summary.revision = ack.revision;
                summary.content_hash = ack.content_hash.clone();
                summary.updated_at = ack.saved_at.clone();
                summary.excerpt = panda_core::derive_excerpt(markdown, 240);
                if let Some(active) = main.active.as_ref().filter(|a| a.id == id) {
                    summary.title = active.title.clone();
                }
                *state = SyncState::Synced;
            }
        }
        main.status = "Synced".into();
        main.error = None;
    }

    fn preserve_local_as_conflict_copy(&mut self, markdown: &str, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let notebook_id = main
            .active
            .as_ref()
            .map(|a| a.notebook_id.clone())
            .unwrap_or_default();
        let conflict_title = format!(
            "{} (Conflict {})",
            main.active.as_ref().map(display_title).unwrap_or("Memo"),
            &Uuid::new_v4().to_string()[..8]
        );
        let client2 = main.client.clone();
        let markdown2 = markdown.to_string();
        cx.spawn(async move |this, cx| {
            let _ = smol::unblock(move || {
                client2.create_memo_blocking(&notebook_id, Some(&conflict_title), &markdown2)
            })
            .await;
            this.update(cx, |this, cx| {
                if let Mode::Main(main) = &mut this.mode {
                    main.status = "Conflict - local copy created".into();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn create_conflict_copy(
        &mut self,
        id: &str,
        markdown: &str,
        err: &panda_api::ApiError,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if let Some(active) = main.active.as_mut() {
            if active.id == id {
                active.sync_state = SyncState::Conflict;
            }
        }
        main.status = "Conflict".into();
        main.error = Some(err.to_string().into());
        self.preserve_local_as_conflict_copy(markdown, cx);
    }

    pub(crate) fn create_memo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let notebook_id = match &main.filter {
            NavFilter::Notebook(id) => id.clone(),
            _ => main
                .notebooks
                .first()
                .map(|n| n.id.clone())
                .unwrap_or_default(),
        };
        if notebook_id.is_empty() {
            main.error = Some("No notebook available".into());
            cx.notify();
            return;
        }
        let client = main.client.clone();
        main.status = "Creating...".into();
        cx.spawn_in(window, async move |this, cx| {
            let created = smol::unblock(move || {
                client.create_memo_blocking(&notebook_id, Some("Untitled"), "")
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match created {
                    Ok(active) => {
                        let id = active.id.clone();
                        if let Mode::Main(main) = &mut this.mode {
                            if !main.memos.iter().any(|(m, _)| m.id == id) {
                                main.memos.insert(
                                    0,
                                    (
                                        MemoSummary {
                                            id: active.id.clone(),
                                            notebook_id: active.notebook_id.clone(),
                                            title: active.title.clone(),
                                            excerpt: panda_core::derive_excerpt(
                                                &active.markdown,
                                                240,
                                            ),
                                            tags: active.tags.clone(),
                                            is_pinned: false,
                                            is_archived: false,
                                            is_deleted: false,
                                            revision: active.revision,
                                            content_hash: active.content_hash.clone(),
                                            etag: active.etag.clone(),
                                            created_at: String::new(),
                                            updated_at: String::new(),
                                            deleted_at: None,
                                        },
                                        SyncState::Synced,
                                    ),
                                );
                            }
                            main.selected_id = Some(id.clone());
                        }
                        this.refresh_remote(window, cx);
                        this.open_memo(&id, window, cx);
                    }
                    Err(e) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(e.to_string().into());
                            main.status = "Create failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn create_journal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let client = main.client.clone();
        let root = main
            .store
            .get_meta("journal_folder")
            .ok()
            .flatten()
            .unwrap_or_else(|| "Journal".into());
        let mut folders = root
            .split(['/', '\\'])
            .map(str::trim)
            .filter(|segment| !segment.is_empty() && *segment != "." && *segment != "..")
            .map(str::to_string)
            .collect::<Vec<_>>();
        if folders.is_empty() {
            main.error = Some("Set a Journal folder in Settings first".into());
            main.status = "Journal folder is required".into();
            cx.notify();
            return;
        }
        let now = chrono::Local::now();
        let title = now.format("%Y%m%d").to_string();
        folders.push(now.format("%Y").to_string());
        folders.push(now.format("%m").to_string());
        main.status = "Preparing journal...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                let mut notebooks = client.list_notebooks_blocking()?;
                let mut parent_id = None;
                for name in folders {
                    let notebook = notebooks
                        .iter()
                        .find(|notebook| {
                            !notebook.is_deleted
                                && notebook.name == name
                                && notebook.parent_id == parent_id
                        })
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| {
                            client.create_notebook_blocking(&name, parent_id.as_deref(), None)
                        })?;
                    parent_id = Some(notebook.id.clone());
                    if !notebooks.iter().any(|existing| existing.id == notebook.id) {
                        notebooks.push(notebook);
                    }
                }
                let notebook_id = parent_id.expect("journal path always has a folder");
                let existing = client
                    .list_memos_blocking(Some(&notebook_id), false, Some(&title), 200)?
                    .into_iter()
                    .find(|memo| memo.title.as_deref() == Some(title.as_str()));
                Ok::<_, panda_api::ApiError>(match existing {
                    Some(memo) => (memo.id, false),
                    None => (
                        client
                            .create_memo_blocking(&notebook_id, Some(&title), "")?
                            .id,
                        true,
                    ),
                })
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok((id, created)) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.status = if created {
                                "Journal created".into()
                            } else {
                                "Journal opened".into()
                            };
                            main.error = None;
                        }
                        this.refresh_remote(window, cx);
                        this.open_memo(&id, window, cx);
                    }
                    Err(error) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(error.to_string().into());
                            main.status = "Journal creation failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(id) = main.selected_id.clone() else {
            main.status = "No memo selected".into();
            cx.notify();
            return;
        };
        let client = main.client.clone();
        let deleted_id = id.clone();
        main.status = "Deleting...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || client.delete_memo_blocking(&id, false)).await;
            this.update_in(cx, |this, window, cx| {
                if let Mode::Main(main) = &mut this.mode {
                    match result {
                        Ok(()) => {
                            let was_open = main.active.as_ref().is_some_and(|a| a.id == deleted_id)
                                || main.selected_id.as_ref() == Some(&deleted_id);
                            main.memos.retain(|(m, _)| m.id != deleted_id);
                            if was_open {
                                main.editor = None;
                                main.active = None;
                                main.selected_id = None;
                                main.title_editor = None;
                                main.preview = None;
                                main._title_sub = None;
                                main._buffer_sub = None;
                                main._preview_sub = None;
                                if let Some(bar) = main.buffer_search_bar.clone() {
                                    bar.update(cx, |bar, cx| {
                                        bar.set_active_pane_item(None, window, cx);
                                    });
                                }
                            }
                            main.status = "Deleted".into();
                            this.refresh_remote(window, cx);
                        }
                        Err(e) => {
                            main.error = Some(e.to_string().into());
                            main.status = "Delete failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_instances_window(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = window.window_handle().downcast::<AppShell>();
        let Some(handle) = handle else {
            return;
        };
        crate::instances_window::InstancesWindow::open(handle, cx);
    }

    pub(crate) fn open_settings_window(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &self.mode else {
            return;
        };
        crate::settings_window::SettingsWindow::open(main.store.clone(), cx);
    }

    pub(crate) fn toggle_preview(&mut self, cx: &mut Context<Self>) {
        let next = match &self.mode {
            Mode::Main(main) => match main.memo_display_mode {
                crate::state::MemoDisplayMode::Source => crate::state::MemoDisplayMode::Live,
                crate::state::MemoDisplayMode::Live => crate::state::MemoDisplayMode::Read,
                crate::state::MemoDisplayMode::Read => crate::state::MemoDisplayMode::Source,
            },
            Mode::Setup(_) => return,
        };
        self.set_memo_display_mode(next, cx);
    }

    pub(crate) fn set_memo_display_mode(
        &mut self,
        mode: crate::state::MemoDisplayMode,
        cx: &mut Context<Self>,
    ) {
        let should_sync_preview = matches!(mode, MemoDisplayMode::Live | MemoDisplayMode::Read);
        if let Mode::Main(main) = &mut self.mode {
            main.memo_display_mode = mode;
        }
        if should_sync_preview {
            self.schedule_preview_update(true, cx);
        }
        cx.notify();
    }

    pub(crate) fn apply_gutter_settings(&mut self, cx: &mut Context<Self>) {
        let Mode::Main(main) = &self.mode else {
            return;
        };
        let gutter = editor::EditorSettings::get_global(cx).gutter;
        if let Some(editor) = main.editor.clone() {
            editor.update(cx, |editor, cx| {
                editor.set_show_gutter(gutter.line_numbers, cx);
                editor.set_show_line_numbers(gutter.line_numbers, cx);
                editor.set_show_breakpoints(gutter.breakpoints, cx);
                editor.set_show_runnables(false, cx);
                editor.set_show_bookmarks(false, cx);
            });
        }
        cx.notify();
    }

    pub(crate) fn toggle_status_bar(&mut self, cx: &mut Context<Self>) {
        self.status_bar_visible = !self.status_bar_visible;
        cx.notify();
    }

    pub(crate) fn toggle_nav_pane(&mut self, cx: &mut Context<Self>) {
        if let Mode::Main(main) = &mut self.mode {
            main.nav_collapsed = !main.nav_collapsed;
            main.persist_nav_collapsed();
        }
        cx.notify();
    }

    pub(crate) fn switch_instance(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(&self.mode, Mode::Main(main) if main.instance.id == id) {
            return;
        }
        if self.session.set_active(id).is_err() {
            return;
        }
        let Some(instance) = self.session.active().cloned() else {
            return;
        };
        self.enter_main(instance, window, cx);
    }

    pub(crate) fn save_memo_now(&mut self, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let memo_id = main.active.as_ref().map(|active| active.id.clone());
        let Some(memo_id) = memo_id else {
            return;
        };
        if let Some(editor) = &main.editor {
            let text = editor.read(cx).text(cx);
            if let Some(active) = main.active.as_mut() {
                active.markdown = text;
                active.sync_state = SyncState::LocalModified;
            }
        }
        main.save_generation += 1;
        self.flush_save(&memo_id, false, cx);
    }

    pub(crate) fn format_memo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(editor) = main.editor.clone() else {
            return;
        };
        let focus = editor.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.dispatch_action(Box::new(editor::actions::Format), cx);
        if let Mode::Main(main) = &mut self.mode {
            main.status = "Format requested".into();
        }
        cx.notify();
    }

    pub(crate) fn commit_title_from_editor(
        &mut self,
        memo_id: &str,
        title_editor: &gpui::Entity<Editor>,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let title = title_editor.read(cx).text(cx).trim().to_string();
        let title = if title.is_empty() { None } else { Some(title) };
        let Some(active) = main.active.as_mut() else {
            return;
        };
        if active.id != memo_id {
            return;
        }
        if active.title == title {
            return;
        }
        active.title = title.clone();
        if let Some((summary, _)) = main.memos.iter_mut().find(|(m, _)| m.id == memo_id) {
            summary.title = title;
        }
        self.schedule_save(memo_id, cx);
    }

    pub(crate) fn move_memo_to_notebook(
        &mut self,
        memo_id: String,
        notebook_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let client = main.client.clone();
        main.status = "Moving...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                client.batch_move_memos_blocking(&[memo_id.clone()], &notebook_id)
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(_) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.status = "Moved".into();
                        }
                        this.refresh_remote(window, cx);
                    }
                    Err(e) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(e.to_string().into());
                            main.status = "Move failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn reorder_notebook(
        &mut self,
        dragged_id: String,
        dragged_parent_id: Option<String>,
        target_id: String,
        target_parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if dragged_id == target_id {
            return;
        }
        if dragged_parent_id != target_parent_id {
            if let Mode::Main(main) = &mut self.mode {
                main.status = "Folders can only be reordered within the same level".into();
            }
            cx.notify();
            return;
        }
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let mut siblings = main
            .notebooks
            .iter()
            .filter(|notebook| !notebook.is_deleted && notebook.parent_id == target_parent_id)
            .map(|notebook| notebook.id.clone())
            .collect::<Vec<_>>();
        siblings.sort_by_key(|id| {
            main.notebooks
                .iter()
                .find(|notebook| &notebook.id == id)
                .map(|notebook| notebook.sort_order)
                .unwrap_or_default()
        });
        let Some(from) = siblings.iter().position(|id| id == &dragged_id) else {
            return;
        };
        let Some(to) = siblings.iter().position(|id| id == &target_id) else {
            return;
        };
        let moved = siblings.remove(from);
        siblings.insert(to, moved);
        let client = main.client.clone();
        main.status = "Reordering folders...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                client.reorder_notebooks_blocking(target_parent_id.as_deref(), &siblings)
            })
            .await;
            this.update_in(cx, |this, _window, cx| {
                if let Mode::Main(main) = &mut this.mode {
                    match result {
                        Ok(notebooks) => {
                            main.notebooks = notebooks;
                            main.status = "Folders reordered".into();
                            main.error = None;
                        }
                        Err(error) => {
                            main.status = "Folder reorder failed".into();
                            main.error = Some(error.to_string().into());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn create_notebook(
        &mut self,
        name: String,
        parent_id: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let client = main.client.clone();
        main.status = "Creating folder...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                client.create_notebook_blocking(&name, parent_id.as_deref(), None)
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(nb) => {
                        let id = nb.id.clone();
                        let name = nb.name.clone();
                        if let Mode::Main(main) = &mut this.mode {
                            main.status = "Folder created".into();
                            if !main.notebooks.iter().any(|n| n.id == id) {
                                main.notebooks.push(nb);
                            }
                        }
                        this.refresh_remote(window, cx);
                        this.begin_rename_notebook(id, window, cx);
                        let _ = name;
                    }
                    Err(e) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(e.to_string().into());
                            main.status = "Create folder failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn delete_notebook(
        &mut self,
        notebook_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if main
            .notebooks
            .iter()
            .any(|n| n.slug.as_deref() == Some("inbox") && n.id == notebook_id)
        {
            main.status = "Cannot delete Inbox".into();
            cx.notify();
            return;
        }
        if main
            .notebooks
            .iter()
            .any(|n| n.parent_id.as_deref() == Some(notebook_id.as_str()))
        {
            main.error = Some("Delete child folders first".into());
            main.status = "Folder not empty".into();
            cx.notify();
            return;
        }
        let memo_ids: Vec<String> = main
            .memos
            .iter()
            .filter(|(m, _)| m.notebook_id == notebook_id && !m.is_deleted)
            .map(|(m, _)| m.id.clone())
            .collect();
        let inbox_id = main.inbox_id();
        let client = main.client.clone();
        let notebook_id_for_filter = notebook_id.clone();
        main.status = "Deleting folder...".into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                if !memo_ids.is_empty() {
                    let Some(inbox_id) = inbox_id else {
                        return Err(panda_api::ApiError::Other(anyhow::anyhow!(
                            "no inbox for move"
                        )));
                    };
                    client.batch_move_memos_blocking(&memo_ids, &inbox_id)?;
                }
                client.delete_notebook_blocking(&notebook_id)
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(()) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.status = "Folder deleted".into();
                            if matches!(
                                &main.filter,
                                NavFilter::Notebook(id) if *id == notebook_id_for_filter
                            ) {
                                main.filter = NavFilter::All;
                            }
                        }
                        this.refresh_remote(window, cx);
                    }
                    Err(e) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(e.to_string().into());
                            main.status = "Delete folder failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn begin_rename_notebook(
        &mut self,
        notebook_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(nb) = main.notebooks.iter().find(|n| n.id == notebook_id) else {
            return;
        };
        let name = nb.name.clone();
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(name, window, cx);
            editor.select_all(&editor::actions::SelectAll, window, cx);
            editor
        });
        window.focus(&editor.focus_handle(cx), cx);
        main.rename_dialog = Some(crate::state::RenameDialog {
            target: crate::state::RenameTarget::Notebook(notebook_id),
            editor,
        });
        cx.notify();
    }

    pub(crate) fn begin_rename_memo(
        &mut self,
        memo_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let title = main
            .memos
            .iter()
            .find(|(m, _)| m.id == memo_id)
            .map(|(m, _)| m.display_title().to_string())
            .unwrap_or_else(|| "Untitled".into());
        let editor = cx.new(|cx| {
            let mut editor = Editor::single_line(window, cx);
            editor.set_text(title, window, cx);
            editor.select_all(&editor::actions::SelectAll, window, cx);
            editor
        });
        window.focus(&editor.focus_handle(cx), cx);
        main.rename_dialog = Some(crate::state::RenameDialog {
            target: crate::state::RenameTarget::Memo(memo_id),
            editor,
        });
        cx.notify();
    }

    pub(crate) fn confirm_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(dialog) = main.rename_dialog.take() else {
            return;
        };
        let name = dialog.editor.read(cx).text(cx).trim().to_string();
        if name.is_empty() {
            cx.notify();
            return;
        }
        match dialog.target {
            crate::state::RenameTarget::Notebook(id) => {
                let client = main.client.clone();
                cx.spawn_in(window, async move |this, cx| {
                    let result =
                        smol::unblock(move || client.rename_notebook_blocking(&id, &name)).await;
                    this.update_in(cx, |this, window, cx| {
                        match result {
                            Ok(_) => {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.status = "Renamed".into();
                                }
                                this.refresh_remote(window, cx);
                            }
                            Err(e) => {
                                if let Mode::Main(main) = &mut this.mode {
                                    main.error = Some(e.to_string().into());
                                    main.status = "Rename failed".into();
                                }
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }
            crate::state::RenameTarget::Memo(id) => {
                if let Some(active) = main.active.as_mut() {
                    if active.id == id {
                        active.title = Some(name.clone());
                    }
                }
                if let Some((summary, _)) = main.memos.iter_mut().find(|(m, _)| m.id == id) {
                    summary.title = Some(name.clone());
                }
                if let Some(title_editor) = &main.title_editor {
                    let name_for_editor = name.clone();
                    title_editor.update(cx, |ed, cx| {
                        ed.set_text(name_for_editor, window, cx);
                    });
                }
                if main.active.as_ref().is_some_and(|a| a.id == id) {
                    self.schedule_save(&id, cx);
                } else {
                    let client = main.client.clone();
                    let title = name;
                    cx.spawn_in(window, async move |this, cx| {
                        let opened = smol::unblock({
                            let id = id.clone();
                            let client = client.clone();
                            move || client.open_memo_blocking(&id)
                        })
                        .await;
                        match opened {
                            Ok(active) => {
                                let save = smol::unblock(move || {
                                    client.save_memo_blocking(
                                        &active.id,
                                        &active.markdown,
                                        &active.etag,
                                        active.revision,
                                        &active.content_hash,
                                        active.lease_id.as_deref(),
                                        Some(&title),
                                        Some(&active.tags),
                                    )
                                })
                                .await;
                                this.update_in(cx, |this, window, cx| {
                                    if let Err(e) = save {
                                        if let Mode::Main(main) = &mut this.mode {
                                            main.error = Some(e.to_string().into());
                                        }
                                    } else if let Mode::Main(main) = &mut this.mode {
                                        main.status = "Renamed".into();
                                    }
                                    this.refresh_remote(window, cx);
                                    cx.notify();
                                })
                                .ok();
                            }
                            Err(e) => {
                                this.update(cx, |this, cx| {
                                    if let Mode::Main(main) = &mut this.mode {
                                        main.error = Some(e.to_string().into());
                                    }
                                    cx.notify();
                                })
                                .ok();
                            }
                        }
                    })
                    .detach();
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        if let Mode::Main(main) = &mut self.mode {
            main.rename_dialog = None;
        }
        cx.notify();
    }

    pub(crate) fn toggle_pin_memo(
        &mut self,
        memo_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some((summary, _)) = main.memos.iter().find(|(m, _)| m.id == memo_id) else {
            return;
        };
        let pinned = !summary.is_pinned;
        let etag = summary.etag.clone();
        let revision = summary.revision;
        let content_hash = summary.content_hash.clone();
        let client = main.client.clone();
        main.status = if pinned { "Pinning..." } else { "Unpinning..." }.into();
        cx.spawn_in(window, async move |this, cx| {
            let result = smol::unblock(move || {
                client.set_memo_pinned_blocking(&memo_id, &etag, revision, &content_hash, pinned)
            })
            .await;
            this.update_in(cx, |this, window, cx| {
                match result {
                    Ok(_) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.status = if pinned { "Pinned" } else { "Unpinned" }.into();
                        }
                        this.refresh_remote(window, cx);
                    }
                    Err(e) => {
                        if let Mode::Main(main) = &mut this.mode {
                            main.error = Some(e.to_string().into());
                            main.status = "Pin failed".into();
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn search_memos_local(
        memos: &[(MemoSummary, SyncState)],
        query: &str,
    ) -> Vec<(MemoSummary, SyncState)> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut hits: Vec<_> = memos
            .iter()
            .filter(|(memo, _)| !memo.is_deleted)
            .filter(|(memo, _)| {
                memo.display_title().to_lowercase().contains(&needle)
                    || memo.excerpt.to_lowercase().contains(&needle)
                    || memo
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&needle))
            })
            .cloned()
            .collect();
        hits.truncate(200);
        hits
    }

    fn merge_search_results(
        local: Vec<(MemoSummary, SyncState)>,
        remote: Vec<MemoSummary>,
    ) -> Vec<(MemoSummary, SyncState)> {
        let mut seen: HashSet<String> = local.iter().map(|(memo, _)| memo.id.clone()).collect();
        let mut merged = local;
        for summary in remote {
            if seen.insert(summary.id.clone()) {
                merged.push((summary, SyncState::Synced));
            }
        }
        merged.truncate(200);
        merged
    }

    fn update_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let trimmed = query.trim().to_string();
        main.search_query = trimmed.clone();
        if trimmed.is_empty() {
            main.search_results = None;
            main.status = format!(
                "{} | {} notebooks | {} memos",
                main.instance.name,
                main.notebooks.len(),
                main.memos.len()
            )
            .into();
            cx.notify();
            return;
        }

        let local = Self::search_memos_local(&main.memos, &trimmed);
        let count = local.len();
        main.search_results = Some(local);
        main.status = format!("Search \"{}\" | {} results", trimmed, count).into();
        cx.notify();

        let client = main.client.clone();
        let query_for_request = trimmed.clone();
        let query_for_state = trimmed.clone();
        cx.spawn(async move |this, cx| {
            let found = smol::unblock(move || {
                client.list_memos_blocking(None, false, Some(&query_for_request), 200)
            })
            .await;
            this.update(cx, |this, cx| {
                let Mode::Main(main) = &mut this.mode else {
                    return;
                };
                if main.search_query != query_for_state {
                    return;
                }
                match found {
                    Ok(remote_items) => {
                        let local = main.search_results.clone().unwrap_or_default();
                        let merged = Self::merge_search_results(local, remote_items);
                        let count = merged.len();
                        main.search_results = Some(merged);
                        main.status =
                            format!("Search \"{}\" | {} results", query_for_state, count).into();
                    }
                    Err(e) => {
                        main.error = Some(e.to_string().into());
                        main.status = "Search failed".into();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_search_result(
        &mut self,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if let Some((memo, _)) = main.memos.iter().find(|(m, _)| m.id == id) {
            main.filter = NavFilter::Notebook(memo.notebook_id.clone());
        }
        main.search_query.clear();
        main.search_results = None;
        if let Some(search_editor) = &main.search_editor {
            search_editor.update(cx, |editor, cx| editor.set_text("", window, cx));
        }
        self.open_memo(id, window, cx);
    }

    pub(crate) fn commit_tag_from_editor(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.add_tag_to_active(text, cx);
        self.clear_tag_editor(window, cx);
    }

    pub(crate) fn clear_tag_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        let Some(tag_editor) = main.tag_editor.clone() else {
            return;
        };
        tag_editor.update(cx, |editor, cx| editor.set_text("", window, cx));
        cx.notify();
    }

    pub(crate) fn remove_tag(&mut self, memo_id: String, tag: String, cx: &mut Context<Self>) {
        let Mode::Main(main) = &mut self.mode else {
            return;
        };
        if let Some(active) = main.active.as_mut() {
            if active.id == memo_id {
                active.tags.retain(|t| t != &tag);
            }
        }
        if let Some((summary, _)) = main.memos.iter_mut().find(|(m, _)| m.id == memo_id) {
            summary.tags.retain(|t| t != &tag);
        }
        self.schedule_save(&memo_id, cx);
    }

    pub(crate) fn add_tag_to_active(&mut self, tag: String, cx: &mut Context<Self>) -> bool {
        let tag = tag.trim();
        if tag.is_empty() {
            return false;
        }
        let Mode::Main(main) = &mut self.mode else {
            return false;
        };
        let Some(active) = main.active.as_mut() else {
            return false;
        };
        if active.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            return false;
        }
        active.tags.push(tag.to_string());
        let memo_id = active.id.clone();
        if let Some((summary, _)) = main.memos.iter_mut().find(|(m, _)| m.id == memo_id) {
            summary.tags = active.tags.clone();
        }
        if !main
            .available_tags
            .iter()
            .any(|available| available.eq_ignore_ascii_case(tag))
        {
            main.available_tags.push(tag.to_string());
            main.available_tags
                .sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
        }
        self.schedule_save(&memo_id, cx);
        cx.notify();
        true
    }
}

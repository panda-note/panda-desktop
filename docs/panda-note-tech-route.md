I revised the previous draft into a technical brief for a “server already exists, three-pane Cloud-native notes client.” When handing this to an Agent, include the server path.

# Panda Note Desktop Client Technical Route

## 1. Project positioning

Panda Note is a **Cloud-native notes application**. As a whole it includes:

* A completed server;
* A desktop client;
* Possible later expansion to mobile or Web.

The server was reimplemented from the EdgeEver design and already provides note data management and sync.

Server code path:

```text
Z:\source\lab\everbook\panda-server
```

Desktop client (Zed fork) code path:

```text
Z:\source\lab\everbook\panda-desktop
```

Local build artifact directory (Z: has limited space; target lives on D:):

```text
D:\cargo-target\panda-desktop
```

This is set by `build.target-dir` in `panda-desktop/.cargo/config.toml`. Do not commit that directory to git.

Extra Windows notes:

* If git checkout hits **path length** limits, set `CARGO_HOME` to a short path (for example `D:\ch`) before building.
* If Spectre-mitigated MSVC libs are missing, the workspace already patches via `crates/msvc_spectre_libs_stub`; you can also install the Spectre components in VS Installer and remove that patch.

Before designing the client data model, API calls, or sync mechanism, an Agent must read the server code and API definitions and must not invent server data structures. Field and API summaries: [panda-server client-adapter](../../panda-server/docs/client-adapter.md).

The main goal of this phase is:

> Based on Zed’s full editor capabilities, build a native, high-performance, three-pane Cloud-native desktop notes client.

---

## 2. Product core goals

This project is not a traditional local Markdown file editor, and it is not a from-scratch code editor.

Core client traits:

1. Server data is the primary business source;
2. Local cache and offline editing are supported;
3. Sync with the server is reliable;
4. The UI is a three-pane notes layout;
5. Zed’s native editor experience is preserved;
6. Mature capabilities such as Vim, multicursor, shortcuts, find/replace, and Undo/Redo are supported;
7. The final product UI must be a pure notes app and must not retain an obvious IDE shape.

Core value:

> Use Zed’s native editing power to build a Cloud-native, high-performance notes client.

---

## 3. Chosen technical route

Follow this route:

```text
Fork Zed
→ Create a standalone crates/panda
→ Keep Zed Editor, Vim, Workspace, and other core capabilities
→ Implement the three-pane notes UI
→ Connect to the existing server
→ Add local cache and sync layers
→ Hide or stop initializing IDE-unrelated features
→ Later trim step by step by dependency
```

Core principle:

> Do not move Zed Editor into an existing client framework; build the notes client on the Zed stack.

---

## 4. Approaches explicitly not taken

Do not adopt these approaches:

1. Do not reimplement the editor with the `gpui-component` Editor;
2. Do not copy Zed’s `editor` crate alone into another project;
3. Do not force-embed Zed Editor into an existing `gpui-component` app;
4. Do not rewrite Vim, multicursor, Selection, Undo, IME, or other core editing capabilities yourself;
5. Do not extract dependencies among Editor, Workspace, Project, and Buffer early in the project;
6. Do not design the client as a mere local Markdown file manager;
7. Do not bypass the existing server and invent another independent data model;
8. Do not treat “delete as much Zed source as possible” as the project goal.

Zed Editor is not a standalone control; it depends on Workspace, Project, Buffer, Language, Settings, Theme, Action, and related systems.

Forcing an extraction causes large amounts of rework and raises the long-term cost of syncing with Zed upstream.

---

## 5. Client UI structure

The client uses a fixed three-pane layout.

```text
┌────────────────┬────────────────────┬──────────────────────────────┐
│ Pane 1         │ Pane 2             │ Pane 3                       │
│ Nav & taxonomy │ Note list          │ Editor                       │
│                │                    │                              │
│ All notes      │ Notes in scope     │ Zed Editor                   │
│ Favorites      │ Title              │                              │
│ Recently       │ Summary            │ Markdown / rich Markdown     │
│ Folders        │ Updated at         │ Vim / multicursor / find     │
│ Tags           │ Sync state         │                              │
│ Trash          │                    │                              │
└────────────────┴────────────────────┴──────────────────────────────┘
```

### Pane 1: Navigation

Pane 1 owns global scope and how notes are organized, including:

* All notes;
* Favorites;
* Recently accessed;
* Folders;
* Tags;
* Trash;
* Possible shared spaces;
* Account and sync-status entry points.

Pane 1 does not edit note bodies.

### Pane 2: Note list

Pane 2 shows notes in the current navigation scope, including:

* Title;
* Summary;
* Updated time;
* Favorite state;
* Tags;
* Sync state;
* Conflict or failure state.

It must support:

* Create note;
* Delete note;
* Rename;
* Search;
* Sort;
* Filter;
* Multi-select;
* Context menu;
* Keyboard navigation.

### Pane 3: Editor

Pane 3 uses Zed Editor as the body-editing core.

Keep:

* Native text rendering;
* Vim mode;
* Multicursor;
* Shortcuts;
* Undo/Redo;
* Find/replace;
* Selection;
* IME;
* Markdown syntax highlighting;
* Soft wrap;
* Large-text editing performance.

Pane 3 also needs to show:

* Note title;
* Save state;
* Sync state;
* Tag or property entry points;
* Conflict hints;
* Possible Markdown preview.

---

## 6. Project structure

Add the following modules to the Zed Cargo workspace (`panda-desktop`). Use the same **flat** `crates/` layout as Zed, distinguished by `panda` / `panda_*` prefixes; do **not** create a nested `crates/panda/{core,api,…}` tree or a repo-root `panda/` subtree.

```text
crates/
├── panda/             # Panda Note desktop client entry
├── panda_core/        # Note domain model
├── panda_api/         # Server API client
├── panda_store/       # Local cache and persistence
├── panda_sync/        # Sync queue, conflict handling, incremental sync
├── panda_index/       # Local full-text index and search
├── panda_ui/          # Three-pane layout and notes business UI
└── panda_session/     # Login state, workspace, and session restore
```

Continue using existing Zed modules:

```text
gpui
ui
editor
vim
workspace
project
worktree
multi_buffer
language
text
rope
settings
theme
search
picker
command_palette
markdown
```

Do not turn `crates/zed` into Panda Note directly.

You must create a standalone application entry:

```text
crates/panda
```

Final build target:

```bash
cargo build --release -p panda
```

Build artifacts go to `D:\cargo-target\panda-desktop` (see `.cargo/config.toml`).

---

## 7. Server-first principle

The server is already done. Client work must follow the existing server implementation.

Before coding, an Agent must:

1. Read the server directory layout;
2. Confirm database entities;
3. Confirm API endpoints;
4. Confirm authentication;
5. Confirm data relationships among notes, folders, and tags;
6. Confirm whether the server supports incremental sync;
7. Confirm whether version, updated time, or Revision exists;
8. Confirm whether deletes are soft or hard;
9. Confirm attachment upload and access;
10. Confirm whether WebSocket, SSE, or change notification already exists.

Server path:

```text
Z:\source\lab\everbook\panda-server
```

Do not invent an incompatible client model without reading the server code.

---

## 8. Recommended data layering

The client must not bind UI directly to the HTTP API.

Use this layering:

```text
UI
↓
panda_core / application service
↓
local repository
↓
sync engine
↓
remote API
```

### 1. Remote data layer

`panda_api` owns:

* Login;
* Token management;
* Fetch notes;
* Create notes;
* Update notes;
* Delete notes;
* Folder and tag operations;
* Attachment upload;
* Incremental sync;
* Fetch server changes.

### 2. Local data layer

`panda_store` owns:

* Local note cache;
* Local edit state;
* Offline data;
* Pending sync operations;
* Recently accessed;
* UI state;
* Search index;
* Failure/retry records.

Prefer SQLite locally.

SQLite is the client local cache and offline database; it does not replace the server database.

### 3. Sync layer

`panda_sync` owns:

* Initial full sync;
* Later incremental sync;
* Upload local changes;
* Pull server changes;
* Offline operation queue;
* Retry after network recovery;
* Delete sync;
* Sync-state maintenance;
* Conflict detection;
* Conflict resolution.

---

## 9. Client data state

Every note should have an explicit local sync state.

```rust
pub enum SyncState {
    Synced,
    LocalCreated,
    LocalModified,
    LocalDeleted,
    Syncing,
    Conflict,
    Failed,
}
```

Suggested client note model:

```rust
pub struct Note {
    pub id: NoteId,
    pub remote_id: Option<String>,
    pub title: String,
    pub content: String,
    pub folder_id: Option<FolderId>,
    pub tags: Vec<TagId>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub remote_revision: Option<String>,
    pub sync_state: SyncState,
}
```

Concrete fields must follow the server code; do not copy this example blindly.

---

## 10. Edit and save strategy

Editor input must not send a network request on every keystroke.

Suggested flow:

```text
User input
→ Update Zed Buffer
→ Short debounce
→ Write local SQLite
→ Mark LocalModified
→ Background sync queue uploads to server
→ Upload succeeds
→ Mark Synced
```

Distinguish three states:

```text
Editor Buffer state
Local database state
Remote server state
```

The client should prioritize:

1. No loss of user input;
2. Local save independent of network;
3. Sync failure does not block editing;
4. Automatic retry after sync recovers;
5. Local data flushed to disk before app exit.

---

## 11. Editor integration principles

Do not change Zed Editor’s core text-editing logic.

Wrap notes at the outer layer:

```rust
pub struct NoteEditor {
    pub editor: Entity<Editor>,
    pub note_id: NoteId,
    pub sync_state: Entity<SyncState>,
    pub metadata: Entity<NoteMetadata>,
}
```

`NoteEditor` owns:

* Current note ID;
* Title;
* Folder;
* Tags;
* Local save;
* Remote sync;
* Sync state;
* Conflict hints;
* Markdown preview;
* Note properties.

Leave all of the following to Zed Editor:

* Text input;
* Cursors and selections;
* Multicursor;
* Vim;
* Undo/Redo;
* Shortcuts;
* Find/replace;
* IME;
* Soft wrap;
* Tree-sitter;
* Text rendering.

---

## 12. Relationship between Buffer and server notes

Zed Editor usually centers on Buffer and the filesystem; this project is Cloud-native notes.

Do not rewrite Editor merely to fit the server.

Prefer one of these strategies:

### Recommended: local mirror file or local Buffer adapter

```text
Server Note
↓
Local cache
↓
Local Markdown mirror or Buffer
↓
Zed Editor
```

The editor only edits the local Buffer.

`panda_store` and `panda_sync` save Buffer content to the local database and sync it to the server.

Whether a real local Markdown mirror file is needed depends on the cost of wiring Zed Buffer.

Priority goals:

* Maximize reuse of Zed Buffer;
* Minimize Editor changes;
* Keep network requests out of Editor core;
* Keep remote data models out of Zed core modules.

---

## 13. Conflict handling

A Cloud-native client must handle multi-device edit conflicts.

After confirming the server implementation, choose a suitable conflict policy.

Minimum requirement:

```text
Compare remote_revision before upload
↓
Same version: normal update
Different version: enter Conflict
```

Never silently overwrite on conflict.

You may offer:

* Keep local version;
* Use remote version;
* Create a conflict copy;
* Open a diff view;
* Merge manually.

Phase 1 can start with:

> On conflict detection, create a conflict copy so no content is lost.

A fuller Diff and merge UI can come later.

---

## 14. Phase 1 development scope

Phase 1 aims for a usable closed loop, not every advanced feature.

### Must complete

```text
Launch app
→ Log in to server
→ Fetch note data
→ Show three-pane UI
→ Navigate categories
→ Show note list
→ Open a note
→ Edit with Zed Editor
→ Local autosave
→ Upload to server
→ Show sync state
```

Also support:

* Create note;
* Edit note;
* Delete note;
* Favorites;
* Folder switching;
* Tag display;
* Search;
* Basic offline editing;
* Retry after network recovery.

### Not required in phase 1

* Real-time multi-user collaboration;
* CRDT;
* Full history versions;
* Rich-text WYSIWYG;
* Complex attachment editing;
* Mobile sync;
* Extension marketplace;
* AI Agent;
* Zed’s original IDE features.

---

## 15. Zed concept mapping

You may reuse Zed infrastructure, but the product must not present IDE semantics.

```text
Zed Workspace       → Panda Note main window
Zed Pane            → Editing area
Zed Editor Item     → Current note editing view
Zed Buffer          → Note body editing cache
Zed Search          → In-note or global search foundation
Zed Outline         → Markdown outline
Zed Settings        → Client settings
Zed Theme           → Client theme
```

Because note bodies come from the server, you cannot simply treat:

```text
Project = all server data
```

Whether `Project` and `Worktree` remain as internal adaptation foundations depends on real Editor init dependencies; they need not be exposed to users.

---

## 16. Handling unrelated features

In phase 1, do not delete Zed source wholesale.

First leave unregistered or hidden:

```text
Agent
Terminal
Git
Debugger
Collaboration
Remote Development
Extension Marketplace
REPL
Tasks
Copilot
Language Model
Project Panel
IDE Status Bar
```

Trim order must be:

```text
Hide UI
→ Stop registering Actions
→ Stop initializing modules
→ Remove panda’s direct dependencies
→ Finally clean Cargo dependencies
```

Do not mass-delete Zed crates at the start.

---

## 17. Trimming goals

Trimming goals for this project:

1. Product UI is a pure three-pane notes app;
2. Unrelated IDE modules are not initialized;
3. Unrelated features do not ship;
4. Startup speed and resource use are reasonable;
5. Zed core changes stay as small as possible;
6. Later sync with Zed upstream remains feasible.

Not required:

* Reducing the source tree to only a few crates;
* Fully removing Workspace and Project;
* Extracting Editor into a standalone generic component;
* Deleting all IDE naming;
* Making the whole Zed repository tiny.

---

## 18. Code change principles

Must follow:

```text
Avoid changing gpui when possible
Avoid changing editor when possible
Avoid changing vim when possible
Change workspace sparingly
Change project sparingly
Put all business features in panda_* crates
```

When Zed modules truly must change:

* Keep the change surface small;
* Commit each change separately;
* Prefer exposing interfaces over rewriting original logic;
* Do not put server business code directly into Zed core modules;
* Do not let `editor` depend directly on `panda_api` or `panda_sync`.

Reasonable change examples:

```text
patch: expose editor constructor
patch: support custom buffer source
patch: expose buffer changed event
patch: support custom workspace item
patch: disable coding service initialization
```

Unreasonable change examples:

```text
editor directly calls note server
editor stores authentication token
workspace directly parses note API response
vim action directly updates remote note
```

---

## 19. Final tech stack

```text
Language:
Rust

Desktop UI:
GPUI + Zed UI + Zed Theme

Editor:
Zed Editor + Zed Vim + MultiBuffer + Tree-sitter

UI:
Three-pane notes layout

Remote service:
Existing EdgeEver-reimplemented server

Local cache:
SQLite

Local search:
SQLite FTS5
Later evaluate Tantivy by data scale

Network:
HTTP chosen to match the server
WebSocket or SSE for change notifications

Sync:
Local-first save
Operation queue
Incremental sync
Failure retry
Version conflict detection
```

---

## 20. Agent execution order

Agents must follow this order strictly.

### Stage 1: Server analysis

```text
Read server code
→ Organize entity model
→ Organize API
→ Organize auth
→ Organize sync and versioning
→ Produce client adapter plan
```

### Stage 2: Minimal Zed validation

```text
Create crates/panda
→ Launch GPUI window
→ Initialize Zed Editor
→ Load a test Markdown
→ Verify Vim and basic editing
```

### Stage 3: Three-pane UI

```text
Implement pane 1 navigation
→ Implement pane 2 note list
→ Embed Zed Editor in pane 3
→ Complete note switching
```

### Stage 4: Local data

```text
Set up SQLite
→ Cache remote notes
→ Persist edits locally
→ Restore last-opened state
```

### Stage 5: Server integration

```text
Login
→ Fetch data
→ Create and update notes
→ Delete notes
→ Show sync state
```

### Stage 6: Offline and conflicts

```text
Offline editing
→ Operation queue
→ Automatic retry
→ Revision checks
→ Conflict copies
```

### Stage 7: Feature trimming

```text
Hide IDE UI
→ Stop initializing unrelated modules
→ Clean panda dependencies
→ Optimize startup speed and install size
```

---

## 21. Final execution principles

Throughout development, always follow:

> Prefer reusing existing server capabilities; do not redesign an incompatible protocol.

> Persist edits reliably locally before remote sync.

> If Zed editing capabilities can be reused, do not reimplement them.

> If an outer wrapper works, do not change Editor core.

> Network, sync, and editor core must stay decoupled.

> Finish the three-pane client and a full sync loop first; deep trimming comes later.

With the server path filled in, this document can serve directly as project-level Agent guidance.
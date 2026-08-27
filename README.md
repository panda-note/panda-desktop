> [!IMPORTANT]
> Remove this line to confirm you've reviewed this PR before submitting.

# Panda Note Desktop

Zed-native three-pane Markdown notes — not an IDE, not a browser shell.

This repository is a [Zed](https://github.com/zed-industries/zed) fork. The
`panda` branch keeps Zed’s editor kernel and adds a notes product shell that
talks to [panda-server](https://github.com/panda-note/panda-server).

## What you get

- Native editing: Vim, multicursor, find/replace, IME
- Three-pane notes UI (nav / list / editor)
- Cloud sync through your own Panda Server (`/api/v1` + WebSocket hints)
- Agent access via the server’s HTTP and MCP surfaces

## What you do not get (yet)

- Multimedia embeds (images / audio / video)
- WYSIWYG editing — Markdown source is canonical

Help wanted: upstream fork engineering, WYSIWYG design, and Zed-native
multimedia. See the project site:
https://panda-note.github.io/panda-note/#help-desktop

## Build

You need a recent Rust toolchain (see `rust-toolchain.toml`) and a normal
native build environment for your OS (MSVC on Windows, Xcode CLT on macOS,
usual C/C++ deps on Linux — same bar as building Zed).

### Release build (default)

There is **no CI packaging** for now (GitHub Actions minutes are limited). Build
locally with the `release-size` profile when you want a shippable binary:
size-first opts, fat LTO, stripped symbols.

```sh
cargo build -p panda --profile release-size
```

Output (path depends on your `CARGO_TARGET_DIR` / Cargo defaults):

```text
target/release-size/panda      # macOS / Linux
target/release-size/panda.exe  # Windows
```

If you publish a GitHub Release later, attach a binary built this way.

### Debug build

Day-to-day iteration:

```sh
cargo build -p panda
```

```text
target/debug/panda
target/debug/panda.exe
```

## Branch model

| Remote / branch | Role |
|-----------------|------|
| `upstream` → `zed-industries/zed` | Zed source of truth |
| `origin/main` | Mirror of Zed `main` (fork default from GitHub) |
| `origin/panda` (default) | Panda Note product branch |

Syncing Zed into `panda` is a repeatable pipeline. Use the project skill
`.agents/skills/panda-upstream-sync/` when merging upstream.

## License

Upstream Zed licensing applies (primarily GPL-3.0-or-later, with Apache-2.0
components where marked). Keep upstream LICENSE and attribution intact.

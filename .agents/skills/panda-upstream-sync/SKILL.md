---
name: panda-upstream-sync
description: >-
  Merge Zed upstream/main into the Panda Note panda branch with a repeatable
  conflict policy, build validation, and push. Use when syncing or merging
  Zed upstream, rebasing panda onto upstream, integrating Zed main, or
  refreshing the panda-desktop fork.
---

# Panda upstream sync

Keep `panda` current with `zed-industries/zed` without losing Panda product
changes. Prefer **merge** over rebase for this fork.

## Remotes and branches

| Ref | Role |
|-----|------|
| `upstream` | `https://github.com/zed-industries/zed.git` |
| `origin` | `https://github.com/panda-note/panda-desktop.git` |
| `panda` | Product branch (default) |
| `main` | Zed tip mirror on the fork |

## Principles

1. **Upstream wins** for Zed-owned infrastructure, refactors, and shared crates.
2. **Panda wins** for `crates/panda*`, `assets/keymaps/panda.json`, Panda theme
   tweaks, CJK Vim (`crates/vim/src/cjk_word.rs` and call sites), Markdown
   authoring actions, and the notes product shell.
3. **Never** resolve shared files with wholesale `-X ours` or `-X theirs`.
   Preserve both intents; adapt Panda code to current upstream APIs.
4. Merge manifests semantically; regenerate or validate `Cargo.lock` — do not
   hand-pick one side.
5. Do not expand the Panda patch surface when upstream already covers the
   behavior.

## Pipeline

Copy this checklist:

```
Upstream sync:
- [ ] 1. Clean tree + remotes
- [ ] 2. Fetch + divergence report
- [ ] 3. Conflict forecast
- [ ] 4. Merge --no-ff --no-commit
- [ ] 5. Review dual-touched files
- [ ] 6. Build + tests
- [ ] 7. Commit merge + push
```

### 1. Clean tree and remotes

```powershell
cd Z:\source\lab\everbook\panda-desktop
git status --short
git remote -v
```

Working tree must be clean (or only known disposable noise). Confirm `upstream`
and `origin` exist.

### 2. Fetch and report divergence

```powershell
git fetch upstream main
git fetch origin
git rev-list --left-right --count panda...upstream/main
git log --oneline -1 upstream/main
git log --oneline -1 panda
```

Record the upstream SHA you are integrating.

### 3. Forecast overlaps

```powershell
git merge-tree --write-tree --messages panda upstream/main
# or:
$base = git merge-base panda upstream/main
git merge-tree $base panda upstream/main
```

List files changed on both sides. Expect review even when Git auto-merges.

### 4. Merge without auto-commit

```powershell
git checkout panda
git merge --no-ff --no-commit upstream/main
```

If conflicts appear, classify each file:

- Zed infrastructure → keep upstream, re-apply Panda call sites if needed
- Panda product files → keep Panda intent
- Shared editor/Vim/language/keymaps → merge by hand; never discard Panda CJK
  or Markdown authoring behavior silently

If product semantics are unclear, **abort** and ask:

```powershell
git merge --abort
```

### 5. Review dual-touched files

At minimum inspect:

- `Cargo.toml`, `Cargo.lock`
- `assets/keymaps/default-*.json`
- `crates/language/**`
- `crates/editor/**`, `crates/vim/**`
- `crates/gpui_windows/**`
- `crates/zed/Cargo.toml`

Confirm `crates/panda*` members remain in the workspace.

### 6. Build and tests

```powershell
$env:CARGO_HOME = 'D:\ch'
$env:CARGO_TARGET_DIR = 'D:\cargo-target\panda-desktop'
cargo build -p panda
```

Expected binary: `D:\cargo-target\panda-desktop\debug\panda.exe`

Run targeted tests for touched Panda/Vim behavior when practical:

```powershell
cargo test -p vim --lib
cargo test -p panda_core --lib
```

Do not finalize the merge commit if the build fails.

Any compatibility fixes required to compile belong in the **merge commit**.

### 7. Commit and push

```powershell
$sha = git rev-parse upstream/main
git commit --trailer "Co-authored-by: Cursor <cursoragent@cursor.com>" --no-edit -m "Merge upstream/main into panda" -m "Integrate Zed tip $sha. Panda product crates and behavior preserved; shared files reviewed and validated with cargo build -p panda."
git push origin panda
```

Optional: refresh the fork `main` mirror:

```powershell
git push origin refs/remotes/upstream/main:refs/heads/main
git branch -f main upstream/main
```

## After sync

- Working tree clean
- `panda` contains the recorded upstream tip
- `cargo build -p panda` still succeeds
- No signing secrets, local logs, or `data/` databases in the commit

## Do not

- Rebase long panda history onto upstream without an explicit user ask
- Force-push `panda` unless the user requests it
- Drop `crates/panda*` or CJK Vim during conflict resolution
- Expand scope into product features unrelated to the sync
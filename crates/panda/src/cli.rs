use std::io::Read as _;

use anyhow::{Context, Result, anyhow};
use clap::{Args, Parser, Subcommand};
use panda_api::ApiClient;
use panda_core::{Instance, MemoSummary, Notebook};
use panda_session::SessionStore;

#[derive(Debug, Parser)]
#[command(name = "panda", version, about = "Panda Note command-line interface")]
struct Cli {
    /// Use a saved server instance by name or ID. Defaults to the active instance.
    #[arg(long, global = true, value_name = "NAME_OR_ID")]
    instance: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Manage notes.
    Memo {
        #[command(subcommand)]
        command: MemoCommand,
    },
    /// Manage notebooks.
    Notebook {
        #[command(subcommand)]
        command: NotebookCommand,
    },
    /// Print the configured server instances.
    Instances,
}

#[derive(Debug, Subcommand)]
enum MemoCommand {
    /// List notes.
    List(MemoList),
    /// Create a note. If no notebook is supplied, use the first available notebook.
    Create(MemoCreate),
    /// Rename a note without changing its content.
    Rename(MemoRename),
    /// Move a note to trash, or permanently delete it with --permanent --yes.
    Delete(MemoDelete),
}

#[derive(Debug, Args)]
struct MemoList {
    /// Filter by notebook ID.
    #[arg(long)]
    notebook: Option<String>,
    /// Search note titles and content.
    #[arg(long)]
    query: Option<String>,
    /// Include notes in the trash.
    #[arg(long)]
    trash: bool,
    /// Maximum number of results (1-200).
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=200))]
    limit: u16,
}

#[derive(Debug, Args)]
struct MemoCreate {
    /// Note title.
    #[arg(long)]
    title: String,
    /// Notebook ID. Defaults to the first notebook on the selected server.
    #[arg(long)]
    notebook: Option<String>,
    /// Markdown body, or `-` to read it from stdin.
    #[arg(long, default_value = "")]
    body: String,
}

#[derive(Debug, Args)]
struct MemoRename {
    /// Note ID.
    id: String,
    /// New title.
    #[arg(long)]
    title: String,
}

#[derive(Debug, Args)]
struct MemoDelete {
    /// Note ID.
    id: String,
    /// Permanently delete rather than moving to trash. Requires --yes.
    #[arg(long)]
    permanent: bool,
    /// Confirm a permanent deletion.
    #[arg(long, requires = "permanent")]
    yes: bool,
}

#[derive(Debug, Subcommand)]
enum NotebookCommand {
    /// List notebooks.
    List,
    /// Create a notebook.
    Create(NotebookCreate),
    /// Rename a notebook.
    Rename(NotebookRename),
    /// Delete a notebook. Requires --yes.
    Delete(NotebookDelete),
}

#[derive(Debug, Args)]
struct NotebookCreate {
    /// Notebook name.
    name: String,
    /// Parent notebook ID.
    #[arg(long)]
    parent: Option<String>,
}

#[derive(Debug, Args)]
struct NotebookRename {
    /// Notebook ID.
    id: String,
    /// New notebook name.
    name: String,
}

#[derive(Debug, Args)]
struct NotebookDelete {
    /// Notebook ID.
    id: String,
    /// Confirm deletion.
    #[arg(long, required = true)]
    yes: bool,
}

pub fn run() {
    #[cfg(target_os = "windows")]
    attach_parent_console();

    if let Err(error) = run_inner() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
fn attach_parent_console() {
    use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AllocConsole, AttachConsole};

    // Release builds use the GUI subsystem to avoid showing a console when launched normally.
    // A console is attached only when the executable is invoked with command-line arguments.
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS).is_err() {
            let _ = AllocConsole();
        }
    }
}

fn run_inner() -> Result<()> {
    let cli = Cli::parse();
    let sessions = SessionStore::load().context("load saved server instances")?;
    if matches!(cli.command, Command::Instances) {
        let active_id = sessions.active().map(|instance| instance.id.as_str());
        for saved in sessions.instances() {
            let active = if Some(saved.id.as_str()) == active_id {
                " (active)"
            } else {
                ""
            };
            println!("{}\t{}\t{}{}", saved.id, saved.name, saved.api_url, active);
        }
        return Ok(());
    }

    let instance = select_instance(&sessions, cli.instance.as_deref())?;
    let api = ApiClient::new(instance.api_url.clone(), instance.token.clone());

    match cli.command {
        Command::Memo { command } => run_memo(&api, command),
        Command::Notebook { command } => run_notebook(&api, command),
        Command::Instances => unreachable!("handled before selecting an instance"),
    }
}

fn select_instance(sessions: &SessionStore, requested: Option<&str>) -> Result<Instance> {
    let instance = if let Some(requested) = requested {
        sessions
            .instances()
            .iter()
            .find(|instance| {
                instance.id == requested || instance.name.eq_ignore_ascii_case(requested)
            })
            .cloned()
            .ok_or_else(|| {
                anyhow!("no saved server instance named or identified as '{requested}'")
            })?
    } else {
        sessions
            .active()
            .cloned()
            .ok_or_else(|| anyhow!("no active server instance; sign in to Panda Note first"))?
    };
    Ok(instance)
}

fn run_memo(api: &ApiClient, command: MemoCommand) -> Result<()> {
    match command {
        MemoCommand::List(args) => {
            let memos = api
                .list_memos_blocking(
                    args.notebook.as_deref(),
                    args.trash,
                    args.query.as_deref(),
                    i64::from(args.limit),
                )
                .map_err(|error| anyhow!(error))?;
            for memo in memos {
                print_memo(&memo);
            }
            Ok(())
        }
        MemoCommand::Create(args) => {
            let notebooks = api
                .list_notebooks_blocking()
                .map_err(|error| anyhow!(error))?;
            let notebook_id = match args.notebook {
                Some(id) => id,
                None => notebooks
                    .first()
                    .map(|notebook| notebook.id.clone())
                    .ok_or_else(|| {
                        anyhow!("no notebooks exist; create one with `panda notebook create`")
                    })?,
            };
            let body = read_body(&args.body)?;
            let memo = api
                .create_memo_blocking(&notebook_id, Some(&args.title), &body)
                .map_err(|error| anyhow!(error))?;
            println!(
                "{}\t{}",
                memo.id,
                memo.title.as_deref().unwrap_or("Untitled")
            );
            Ok(())
        }
        MemoCommand::Rename(args) => {
            let memo = api
                .open_memo_blocking(&args.id)
                .map_err(|error| anyhow!(error))?;
            api.save_memo_blocking(
                &memo.id,
                &memo.markdown,
                &memo.etag,
                memo.revision,
                &memo.content_hash,
                memo.lease_id.as_deref(),
                Some(&args.title),
                Some(&memo.tags),
            )
            .map_err(|error| anyhow!(error))?;
            println!("{}\t{}", memo.id, args.title);
            Ok(())
        }
        MemoCommand::Delete(args) => {
            api.delete_memo_blocking(&args.id, args.permanent)
                .map_err(|error| anyhow!(error))?;
            println!(
                "{}",
                if args.permanent {
                    "Permanently deleted"
                } else {
                    "Moved to trash"
                }
            );
            Ok(())
        }
    }
}

fn run_notebook(api: &ApiClient, command: NotebookCommand) -> Result<()> {
    match command {
        NotebookCommand::List => {
            for notebook in api
                .list_notebooks_blocking()
                .map_err(|error| anyhow!(error))?
            {
                print_notebook(&notebook);
            }
            Ok(())
        }
        NotebookCommand::Create(args) => {
            let notebook = api
                .create_notebook_blocking(&args.name, args.parent.as_deref(), None)
                .map_err(|error| anyhow!(error))?;
            print_notebook(&notebook);
            Ok(())
        }
        NotebookCommand::Rename(args) => {
            let notebook = api
                .rename_notebook_blocking(&args.id, &args.name)
                .map_err(|error| anyhow!(error))?;
            print_notebook(&notebook);
            Ok(())
        }
        NotebookCommand::Delete(args) => {
            api.delete_notebook_blocking(&args.id)
                .map_err(|error| anyhow!(error))?;
            println!("Deleted notebook {}", args.id);
            Ok(())
        }
    }
}

fn read_body(body: &str) -> Result<String> {
    if body != "-" {
        return Ok(body.to_string());
    }
    let mut body = String::new();
    std::io::stdin()
        .read_to_string(&mut body)
        .context("read Markdown from stdin")?;
    Ok(body)
}

fn print_memo(memo: &MemoSummary) {
    println!(
        "{}\t{}\t{}",
        memo.id,
        memo.display_title().replace(['\t', '\n', '\r'], " "),
        memo.notebook_id
    );
}

fn print_notebook(notebook: &Notebook) {
    println!(
        "{}\t{}\t{}",
        notebook.id,
        notebook.path.replace(['\t', '\n', '\r'], " "),
        notebook.memo_count
    );
}

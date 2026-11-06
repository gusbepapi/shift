use std::path::PathBuf;

use clap::{Args, Subcommand, ValueHint};

use super::MutationArgs;

#[derive(Debug, Subcommand)]
pub(crate) enum PageCommand {
    /// List the pages of the document's canvas with their text runs.
    List(ListPagesArgs),
    /// Add an empty page after the existing ones.
    Add(AddPageArgs),
    /// Rename a page.
    Rename(RenamePageArgs),
    /// Remove a page with everything placed on it.
    Remove(RemovePageArgs),
}

#[derive(Debug, Args)]
pub(crate) struct ListPagesArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Emit a structured result for scripts and agents.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub(crate) struct AddPageArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Name shown in the pages list.
    pub name: String,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

#[derive(Debug, Args)]
pub(crate) struct RenamePageArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Page name or full stable node id.
    pub page: String,

    /// The new name.
    pub name: String,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

#[derive(Debug, Args)]
pub(crate) struct RemovePageArgs {
    /// Path to the canonical SQLite .shift document.
    #[arg(value_hint = ValueHint::FilePath)]
    pub path: PathBuf,

    /// Page name or full stable node id.
    pub page: String,

    #[command(flatten)]
    pub mutation: MutationArgs,
}

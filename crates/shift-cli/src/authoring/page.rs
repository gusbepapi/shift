//! Canvas page authoring: the pages of a document's scene, and what they hold.

use std::collections::BTreeMap;
use std::path::Path;

use miette::{IntoDiagnostic, Result, WrapErr, bail, miette};
use serde::Serialize;
use shift_engine::DocumentChangeSet;
use shift_engine::scene::{DeleteNode, NodeContent, Scene, SceneIntent, SceneNode, TextItem};
use shift_font::{FontChangeSet, index_key};
use shift_store::{SaveState, ShiftStore};
use shift_workspace::FontWorkspace;

use super::{AuthoringChange, AuthoringReport, mutation_destination};
use crate::Report;
use crate::cli::{AddPageArgs, ListPagesArgs, MutationArgs, RemovePageArgs, RenamePageArgs};

/// One page of the canvas and what is placed on it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageSummary {
    pub page_id: String,
    pub name: String,
    pub node_count: usize,
    pub text_runs: Vec<TextRunSummary>,
}

/// A line of proof text on a page, with how it is shaped.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TextRunSummary {
    pub node_id: String,
    pub size: f64,
    pub item_count: usize,
    pub shaping: bool,
    pub features: BTreeMap<String, bool>,
    pub language: Option<String>,
    pub script: Option<String>,
}

/// The pages of a document, in list order.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageListReport {
    pub document: String,
    pub pages: Vec<PageSummary>,
}

impl PageListReport {
    fn of(document: &Path, scene: &Scene) -> Self {
        let pages = scene
            .pages()
            .map(|page| {
                let descendants = scene.descendants(&page.id);
                let text_runs = descendants
                    .iter()
                    .copied()
                    .filter_map(text_run_summary)
                    .collect();

                PageSummary {
                    page_id: page.id.to_string(),
                    name: page_name(page).to_string(),
                    node_count: descendants.len(),
                    text_runs,
                }
            })
            .collect();

        Self {
            document: document.display().to_string(),
            pages,
        }
    }
}

impl Report for PageListReport {
    fn render(&self) -> String {
        let mut lines = vec![self.document.clone(), String::new()];
        for page in &self.pages {
            lines.push(format!(
                "{}  {}  {} nodes",
                page.page_id, page.name, page.node_count
            ));
            for run in &page.text_runs {
                let shaping = if run.shaping { "shaped" } else { "unshaped" };
                let language = run.language.as_deref().unwrap_or("-");
                let script = run.script.as_deref().unwrap_or("-");
                lines.push(format!(
                    "  {}  run  {} items  size {}  {shaping}  language {language}  script {script}",
                    run.node_id, run.item_count, run.size
                ));
            }
        }

        lines.join("\n")
    }
}

fn text_run_summary(node: &SceneNode) -> Option<TextRunSummary> {
    let NodeContent::TextRun {
        size,
        items,
        settings,
        ..
    } = &node.content
    else {
        return None;
    };
    let item_count = items
        .iter()
        .filter(|item| !matches!(item, TextItem::Linebreak { .. }))
        .count();

    Some(TextRunSummary {
        node_id: node.id.to_string(),
        size: *size,
        item_count,
        shaping: settings.shaping,
        features: settings.features.clone(),
        language: settings.language.clone(),
        script: settings.script.clone(),
    })
}

fn page_name(page: &SceneNode) -> &str {
    match &page.content {
        NodeContent::Page { name, .. } => name,
        _ => "",
    }
}

/// Lists the document's pages and the proof text on them.
///
/// # Errors
///
/// Returns an error when the document cannot be opened or its scene read.
pub(crate) fn list_pages(args: &ListPagesArgs) -> Result<PageListReport> {
    let scene = ShiftStore::open_document(&args.path)
        .and_then(|store| store.load_scene())
        .into_diagnostic()
        .wrap_err("failed to read the document's scene")?;

    Ok(PageListReport::of(&args.path, &scene))
}

/// Adds an empty page after the existing ones.
///
/// # Errors
///
/// Returns an error for a blank name or a document/persistence failure.
pub(crate) fn add_page(args: &AddPageArgs) -> Result<AuthoringReport> {
    let name = args.name.trim();
    if name.is_empty() {
        bail!("a page name must not be blank");
    }
    apply_scene_mutation(&args.path, &args.mutation, |scene| {
        let last = scene.pages().last().map(|page| page.index.clone());
        let mut page = SceneNode::new_page(name).into_diagnostic()?;
        page.index = index_key::between(last.as_deref(), None).into_diagnostic()?;
        let change = AuthoringChange::PageCreated {
            page_id: page.id.to_string(),
            name: name.to_string(),
        };

        Ok((vec![SceneIntent::CreateNode(page)], change))
    })
}

/// Renames a page.
///
/// # Errors
///
/// Returns an error for an unknown or ambiguous page, a blank name, or a
/// document/persistence failure.
pub(crate) fn rename_page(args: &RenamePageArgs) -> Result<AuthoringReport> {
    let name = args.name.trim();
    if name.is_empty() {
        bail!("a page name must not be blank");
    }
    apply_scene_mutation(&args.path, &args.mutation, |scene| {
        let page = select_page(scene, &args.page)?;
        let mut renamed = page.clone();
        renamed.content = NodeContent::Page {
            name: name.to_string(),
            role: match &page.content {
                NodeContent::Page { role, .. } => *role,
                _ => None,
            },
        };
        let change = AuthoringChange::PageRenamed {
            page_id: page.id.to_string(),
            from: page_name(page).to_string(),
            name: name.to_string(),
        };

        Ok((vec![SceneIntent::UpdateNode(renamed)], change))
    })
}

/// Removes a page with everything placed on it.
///
/// # Errors
///
/// Returns an error for an unknown or ambiguous page, for the last page, or a
/// document/persistence failure.
pub(crate) fn remove_page(args: &RemovePageArgs) -> Result<AuthoringReport> {
    apply_scene_mutation(&args.path, &args.mutation, |scene| {
        let page = select_page(scene, &args.page)?;
        if scene.pages().count() == 1 {
            bail!("the last page cannot be removed");
        }
        let change = AuthoringChange::PageRemoved {
            page_id: page.id.to_string(),
            name: page_name(page).to_string(),
            node_count: scene.descendants(&page.id).len(),
        };

        Ok((
            vec![SceneIntent::DeleteNode(DeleteNode {
                id: page.id.clone(),
            })],
            change,
        ))
    })
}

/// The page named `selector` or carrying it as its id.
fn select_page<'scene>(scene: &'scene Scene, selector: &str) -> Result<&'scene SceneNode> {
    if let Some(page) = scene.pages().find(|page| page.id.to_string() == selector) {
        return Ok(page);
    }
    let mut named = scene.pages().filter(|page| page_name(page) == selector);
    let Some(page) = named.next() else {
        return Err(miette!("no page is named or identified {selector:?}"));
    };
    if named.next().is_some() {
        bail!("several pages are named {selector:?}; use a page id");
    }

    Ok(page)
}

/// Opens the document, applies the scene intents `plan` builds from its scene
/// and writes it like every other authoring command.
fn apply_scene_mutation(
    path: &Path,
    options: &MutationArgs,
    plan: impl FnOnce(&Scene) -> Result<(Vec<SceneIntent>, AuthoringChange)>,
) -> Result<AuthoringReport> {
    let destination = mutation_destination(path, options.output.as_deref())?;
    let recovery = tempfile::tempdir()
        .into_diagnostic()
        .wrap_err("failed to create temporary recovery directory")?;
    let mut workspace = FontWorkspace::open_document(path, recovery.path().join("recovery.sqlite"))
        .into_diagnostic()
        .wrap_err("failed to open Shift document")?;
    let (intents, change) = plan(workspace.scene())?;
    let mut scene = workspace.scene().clone();
    let scene_changes = scene
        .apply_intents(&intents)
        .into_diagnostic()
        .wrap_err("page change is invalid")?;
    let change_set = DocumentChangeSet {
        font: FontChangeSet::default(),
        scene: scene_changes,
    };
    workspace
        .persist_compact_change_set(change_set.compact(), SaveState::Unsaved)
        .into_diagnostic()
        .wrap_err("failed to apply the page change")?;

    if !options.dry_run {
        if options.output.is_some() {
            workspace
                .save_as_document(&destination, recovery.path().join("output-recovery.sqlite"))
                .into_diagnostic()
                .wrap_err("failed to save independent Shift font")?;
        } else {
            workspace
                .save()
                .into_diagnostic()
                .wrap_err("failed to save Shift font")?;
        }
    }

    Ok(AuthoringReport {
        valid: true,
        document: path.to_path_buf(),
        output: destination,
        wrote: !options.dry_run,
        changes: vec![change],
    })
}

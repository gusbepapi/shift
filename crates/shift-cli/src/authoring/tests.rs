use std::path::Path;

use shift_font::Font;
use shift_store::{DocumentId, ShiftStore};

use crate::cli::{
    AddAxisArgs, AddPageArgs, AddSourceArgs, CreateFontArgs, ListPagesArgs, MutationArgs,
    RemovePageArgs, RenamePageArgs,
};

use super::{add_axis, add_page, add_source, create_font, list_pages, remove_page, rename_page};

fn mutation(dry_run: bool) -> MutationArgs {
    MutationArgs {
        output: None,
        dry_run,
        json: false,
    }
}

fn create_document(path: &Path) {
    create_font(CreateFontArgs {
        path: path.to_path_buf(),
        from: None,
        dry_run: false,
        json: false,
    })
    .unwrap();
}

fn load_font(path: &Path) -> Font {
    ShiftStore::open_document(path)
        .unwrap()
        .load_font_state()
        .unwrap()
}

fn document_id(path: &Path) -> DocumentId {
    ShiftStore::open_document(path)
        .unwrap()
        .document_metadata()
        .unwrap()
        .document_id
}

fn weight_axis(path: &Path, mutation: MutationArgs) -> AddAxisArgs {
    AddAxisArgs {
        path: path.to_path_buf(),
        tag: "wght".to_string(),
        name: "Weight".to_string(),
        minimum: 100.0,
        default: 400.0,
        maximum: 900.0,
        mutation,
    }
}

#[test]
fn create_font_writes_a_new_document_and_refuses_to_overwrite_it() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Lab.shift");

    let report = create_font(CreateFontArgs {
        path: path.clone(),
        from: None,
        dry_run: false,
        json: false,
    })
    .unwrap();

    assert!(report.wrote);
    assert_eq!(load_font(&path).sources().len(), 1);
    assert!(
        create_font(CreateFontArgs {
            path,
            from: None,
            dry_run: false,
            json: false,
        })
        .is_err()
    );
}

#[test]
fn axis_dry_run_uses_real_validation_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Lab.shift");
    create_document(&path);
    let before_id = document_id(&path);

    let report = add_axis(weight_axis(&path, mutation(true))).unwrap();

    assert!(!report.wrote);
    assert_eq!(document_id(&path), before_id);
    assert!(load_font(&path).axes().is_empty());
}

#[test]
fn axis_mutation_preserves_document_identity() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Lab.shift");
    create_document(&path);
    let original_id = document_id(&path);

    add_axis(weight_axis(&path, mutation(false))).unwrap();

    assert_eq!(document_id(&path), original_id);
    assert_eq!(load_font(&path).axes()[0].tag(), "wght");
}

#[test]
fn invalid_axis_does_not_change_the_document() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Lab.shift");
    create_document(&path);
    let before = load_font(&path);
    let mut args = weight_axis(&path, mutation(false));
    args.minimum = 500.0;
    args.default = 400.0;

    add_axis(args).unwrap_err();
    assert_eq!(load_font(&path), before);
}

#[test]
fn output_writes_an_independent_document_without_changing_input() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("Lab.shift");
    let output = temp.path().join("Variant.shift");
    create_document(&input);
    let before = load_font(&input);
    let mut options = mutation(false);
    options.output = Some(output.clone());

    add_axis(weight_axis(&input, options)).unwrap();

    assert_eq!(load_font(&input), before);
    assert_eq!(load_font(&output).axes().len(), 1);
    assert_ne!(document_id(&input), document_id(&output));
}

#[test]
fn source_location_is_completed_with_axis_defaults() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Lab.shift");
    create_document(&path);
    add_axis(weight_axis(&path, mutation(false))).unwrap();

    add_source(AddSourceArgs {
        path: path.clone(),
        name: "Black".to_string(),
        location: vec!["wght=900".to_string()],
        mutation: mutation(false),
    })
    .unwrap();

    let font = load_font(&path);
    let source = font
        .sources()
        .iter()
        .find(|source| source.name() == "Black")
        .unwrap();
    assert_eq!(source.location().get(&font.axes()[0].id()), Some(900.0));
}

fn page_names(path: &Path) -> Vec<String> {
    list_pages(&ListPagesArgs {
        path: path.to_path_buf(),
        json: false,
    })
    .unwrap()
    .pages
    .into_iter()
    .map(|page| page.name)
    .collect()
}

fn add_named_page(path: &Path, name: &str, mutation: MutationArgs) -> super::AuthoringReport {
    add_page(&AddPageArgs {
        path: path.to_path_buf(),
        name: name.to_string(),
        mutation,
    })
    .unwrap()
}

#[test]
fn new_document_lists_one_empty_page() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Pages.shift");
    create_document(&path);

    let report = list_pages(&ListPagesArgs { path, json: false }).unwrap();

    assert_eq!(report.pages.len(), 1);
    assert_eq!(report.pages[0].node_count, 0);
    assert!(report.pages[0].text_runs.is_empty());
}

#[test]
fn page_lifecycle_persists_add_rename_and_remove() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Pages.shift");
    create_document(&path);

    add_named_page(&path, "Proofs", mutation(false));
    assert_eq!(page_names(&path), ["Page 1", "Proofs"]);

    rename_page(&RenamePageArgs {
        path: path.clone(),
        page: "Proofs".to_string(),
        name: "Specimens".to_string(),
        mutation: mutation(false),
    })
    .unwrap();
    assert_eq!(page_names(&path), ["Page 1", "Specimens"]);

    remove_page(&RemovePageArgs {
        path: path.clone(),
        page: "Specimens".to_string(),
        mutation: mutation(false),
    })
    .unwrap();
    assert_eq!(page_names(&path), ["Page 1"]);
}

#[test]
fn page_dry_run_validates_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Pages.shift");
    create_document(&path);

    let report = add_named_page(&path, "Proofs", mutation(true));

    assert!(!report.wrote);
    assert_eq!(page_names(&path), ["Page 1"]);
}

#[test]
fn page_output_leaves_the_input_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Pages.shift");
    let output = temp.path().join("Copy.shift");
    create_document(&path);

    add_named_page(
        &path,
        "Proofs",
        MutationArgs {
            output: Some(output.clone()),
            dry_run: false,
            json: false,
        },
    );

    assert_eq!(page_names(&path), ["Page 1"]);
    assert_eq!(page_names(&output), ["Page 1", "Proofs"]);
}

#[test]
fn page_commands_refuse_blank_unknown_and_last_pages() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("Pages.shift");
    create_document(&path);

    let blank = add_page(&AddPageArgs {
        path: path.clone(),
        name: "  ".to_string(),
        mutation: mutation(false),
    });
    let unknown = rename_page(&RenamePageArgs {
        path: path.clone(),
        page: "Missing".to_string(),
        name: "Other".to_string(),
        mutation: mutation(false),
    });
    let last = remove_page(&RemovePageArgs {
        path: path.clone(),
        page: "Page 1".to_string(),
        mutation: mutation(false),
    });

    assert!(blank.is_err());
    assert!(unknown.is_err());
    assert!(last.unwrap_err().to_string().contains("last page"));
    assert_eq!(page_names(&path), ["Page 1"]);
}

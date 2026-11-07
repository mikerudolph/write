use tempfile::tempdir;
use std::collections::HashSet;
use write_store::{title_from, Library, StoreError, EMBED_ANCHOR};

fn typed(text: &str, document: &write_store::Document) {
    let mut position = 0;
    for character in text.chars() {
        document.edit(position, 0, &character.to_string()).unwrap();
        position += character.len_utf16() as u64;
    }
}

#[test]
fn every_edit_survives_without_a_save_or_close() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let id = {
        let library = Library::open(&path).unwrap();
        let document = library.create().unwrap();
        typed("Hello 東京 😀", &document);
        let id = document.id();
        std::mem::forget(document);
        std::mem::forget(library);
        id
    };
    let library = Library::open(&path).unwrap();
    assert_eq!(library.open_document(&id).unwrap().text(), "Hello 東京 😀");
}

#[test]
fn uses_utf16_positions() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "😀b").unwrap();
    document.edit(2, 0, "a").unwrap();
    assert_eq!(document.text(), "😀ab");
    document.edit(0, 2, "").unwrap();
    assert_eq!(document.text(), "ab");
}

#[test]
fn replacing_a_selection_is_one_edit() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "one two three").unwrap();
    document.edit(4, 3, "2").unwrap();
    assert_eq!(document.text(), "one 2 three");
}

#[test]
fn rejects_a_range_outside_the_text() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "abc").unwrap();
    assert!(matches!(document.edit(2, 5, ""), Err(StoreError::Position { .. })));
    assert!(matches!(document.edit(u64::MAX, 1, ""), Err(StoreError::Position { .. })));
    assert_eq!(document.text(), "abc");
}

#[test]
fn reconcile_repairs_a_difference_and_keeps_surrogate_pairs() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "a😀b").unwrap();
    assert!(!document.reconcile("a😀b").unwrap());
    assert!(document.reconcile("a😁b").unwrap());
    assert_eq!(document.text(), "a😁b");
    assert!(document.reconcile("").unwrap());
    assert_eq!(document.text(), "");
    assert!(document.reconcile("😀😀").unwrap());
    assert!(document.reconcile("😀").unwrap());
    assert_eq!(document.text(), "😀");
}

#[test]
fn compaction_keeps_the_text_and_later_edits() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    let id = document.id();
    typed("before ", &document);
    document.compact().unwrap();
    document.edit(7, 0, "after").unwrap();
    let reopened = library.open_document(&id).unwrap();
    assert_eq!(reopened.text(), "before after");
}

#[test]
fn opening_a_long_history_compacts_it() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    let id = document.id();
    let text = "x".repeat(600);
    typed(&text, &document);
    let reopened = library.open_document(&id).unwrap();
    assert_eq!(reopened.text(), text);
    let third = library.open_document(&id).unwrap();
    assert_eq!(third.text(), text);
}

#[test]
fn documents_are_separate_and_the_list_shows_titles() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let first = library.create().unwrap();
    let second = library.create().unwrap();
    first.edit(0, 0, "# Trip notes\n\nBody").unwrap();
    second.edit(0, 0, "Book outline").unwrap();
    assert_eq!(first.text(), "# Trip notes\n\nBody");
    let titles: Vec<String> = library.list().unwrap().into_iter().map(|item| item.title).collect();
    assert_eq!(titles.len(), 2);
    assert!(titles.contains(&"Trip notes".to_string()));
    assert!(titles.contains(&"Book outline".to_string()));
}

#[test]
fn an_unknown_document_gives_an_error() {
    let directory = tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    assert!(matches!(library.open_document("nothing"), Err(StoreError::NotFound(_))));
}

#[test]
fn a_title_comes_from_the_first_text_line() {
    assert_eq!(title_from("\n\n## Heading\nBody"), "Heading");
    assert_eq!(title_from(""), "Untitled");
    assert_eq!(title_from("#"), "Untitled");
}

fn ids(document: &write_store::Document) -> Vec<String> {
    document.blocks().into_iter().map(|block| block.id).collect()
}

fn new_document(directory: &tempfile::TempDir) -> (Library, std::sync::Arc<write_store::Document>) {
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    (library, document)
}

#[test]
fn every_block_has_a_unique_id_and_correct_range() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    document.edit(0, 0, "one\n😀two\nthree").unwrap();
    let blocks = document.blocks();
    assert_eq!(blocks.len(), 3);
    assert_eq!((blocks[0].start, blocks[0].end), (0, 3));
    assert_eq!((blocks[1].start, blocks[1].end), (4, 9));
    assert_eq!((blocks[2].start, blocks[2].end), (10, 15));
    assert_eq!(ids(&document).iter().collect::<HashSet<_>>().len(), 3);
    assert!(blocks.iter().all(|block| !block.id.is_empty() && block.kind == "text"));
}

#[test]
fn typing_inside_a_block_keeps_its_id() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    document.edit(0, 0, "one\ntwo").unwrap();
    let before = ids(&document);
    document.edit(1, 0, "x").unwrap();
    document.edit(6, 0, "y").unwrap();
    assert_eq!(ids(&document), before);
}

#[test]
fn enter_at_the_end_of_a_paragraph_keeps_its_id() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    document.edit(0, 0, "one\ntwo").unwrap();
    let before = ids(&document);
    document.edit(3, 0, "\n").unwrap();
    let after = ids(&document);
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[1]);
    assert_eq!(after.iter().collect::<HashSet<_>>().len(), 3);
}

#[test]
fn enter_at_the_end_of_the_document_keeps_the_id() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    typed("one", &document);
    let before = ids(&document);
    document.edit(3, 0, "\n").unwrap();
    let after = ids(&document);
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[0]);
}

#[test]
fn enter_inside_a_paragraph_gives_two_ids() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    document.edit(0, 0, "hello world\nnext").unwrap();
    let before = ids(&document);
    document.edit(6, 0, "\n").unwrap();
    let after = ids(&document);
    assert_eq!(after.len(), 3);
    assert_eq!(after[1], before[0]);
    assert_eq!(after[2], before[1]);
}

#[test]
fn pasted_lines_get_unique_ids() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    document.edit(0, 0, "ab").unwrap();
    document.edit(1, 0, "1\n2\n\n3").unwrap();
    assert_eq!(document.text(), "a1\n2\n\n3b");
    let found = ids(&document);
    assert_eq!(found.len(), 4);
    assert_eq!(found.iter().collect::<HashSet<_>>().len(), 4);
}

#[test]
fn ids_survive_a_reload_and_compaction() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "one\ntwo\nthree").unwrap();
    let before = ids(&document);
    let id = document.id();
    assert_eq!(ids(&library.open_document(&id).unwrap()), before);
    document.compact().unwrap();
    assert_eq!(ids(&library.open_document(&id).unwrap()), before);
}

#[test]
fn an_embed_keeps_its_source_across_edits_and_reloads() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "intro\n\noutro").unwrap();
    let embed = document.insert_embed(6, "table", "| a |").unwrap();
    assert_eq!(document.text(), format!("intro\n{EMBED_ANCHOR}\noutro"));
    let blocks = document.blocks();
    assert_eq!(blocks[1].kind, "embed");
    assert_eq!(blocks[0].kind, "text");
    document.edit_embed(&embed, 5, 0, " b").unwrap();
    let reopened = library.open_document(&document.id()).unwrap();
    assert_eq!(reopened.embed_source(&embed).as_deref(), Some("| a | b"));
    assert_eq!(reopened.blocks()[1].kind, "embed");
    assert!(reopened.edit_embed("missing", 0, 0, "x").is_err());
}

#[test]
fn a_title_ignores_an_embed_anchor() {
    assert_eq!(title_from(&format!("{EMBED_ANCHOR}\nNotes")), "Notes");
}

#[test]
fn large_documents_keep_ids_after_many_lines() {
    let directory = tempdir().unwrap();
    let (_library, document) = new_document(&directory);
    let text = "line of text\n".repeat(5000);
    document.edit(0, 0, &text).unwrap();
    let found = ids(&document);
    assert_eq!(found.len(), 5001);
    assert_eq!(found.iter().collect::<HashSet<_>>().len(), 5001);
}

fn raw_document(path: &std::path::Path, id: &str, build: impl Fn(&loro::LoroDoc)) {
    let document = loro::LoroDoc::new();
    build(&document);
    document.commit();
    let bytes = document.export(loro::ExportMode::Snapshot).unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute(
            "INSERT INTO documents (id, format, title, created_ms, updated_ms, snapshot) VALUES (?1, 1, 'Old', 0, 0, ?2)",
            rusqlite::params![id, bytes],
        )
        .unwrap();
}

#[test]
fn an_old_document_gains_ids_when_it_opens() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    raw_document(&path, "old", |document| {
        document.get_text("body").insert(0, "one\ntwo\nthree").unwrap();
    });
    let opened = library.open_document("old").unwrap();
    assert_eq!(opened.text(), "one\ntwo\nthree");
    let first = ids(&opened);
    assert_eq!(first.iter().filter(|id| !id.is_empty()).collect::<HashSet<_>>().len(), 3);
    assert_eq!(ids(&library.open_document("old").unwrap()), first);
    let connection = rusqlite::Connection::open(&path).unwrap();
    let format: i64 = connection
        .query_row("SELECT format FROM documents WHERE id = 'old'", [], |row| row.get(0))
        .unwrap();
    assert_eq!(format, 2);
}

#[test]
fn a_duplicate_id_is_repaired_when_the_document_opens() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    raw_document(&path, "twice", |document| {
        let mut styles = loro::StyleConfigMap::new();
        styles.insert("block".into(), loro::StyleConfig { expand: loro::ExpandType::None });
        document.config_text_style(styles);
        let text = document.get_text("body");
        text.insert(0, "one\ntwo\nthree").unwrap();
        text.mark(3..4, "block", "same").unwrap();
        text.mark(7..8, "block", "same").unwrap();
    });
    let found = ids(&library.open_document("twice").unwrap());
    assert_eq!(found[0], "same");
    assert_ne!(found[1], "same");
    assert_eq!(found.iter().collect::<HashSet<_>>().len(), 3);
}

#[test]
fn embeds_report_their_positions_and_sources() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "😀 start\n\nend").unwrap();
    let id = document.insert_embed(10, "math", "x^2").unwrap();

    let embeds = document.embeds();
    assert_eq!(embeds.len(), 1);
    assert_eq!(embeds[0].id, id);
    assert_eq!(embeds[0].kind, "math");
    assert_eq!(embeds[0].source, "x^2");
    assert_eq!(embeds[0].position, 10);
}

#[test]
fn a_restored_anchor_gets_its_embed_again() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "a\n\nb").unwrap();
    let id = document.insert_embed(2, "table", "| x |").unwrap();

    document.edit(2, 1, "").unwrap();
    assert!(document.embeds().is_empty());
    assert_eq!(document.embed_source(&id).as_deref(), Some("| x |"));

    document.edit(2, 0, "\u{FFFC}").unwrap();
    document.attach_embed(2, &id).unwrap();
    assert_eq!(document.embeds().len(), 1);

    drop(document);
    drop(library);
    let reopened = Library::open(&path).unwrap();
    let id_list = reopened.list().unwrap();
    let document = reopened.open_document(&id_list[0].id).unwrap();
    assert_eq!(document.embeds()[0].id, id);
}

#[test]
fn an_embed_attaches_only_to_an_anchor_and_a_known_id() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "ab").unwrap();
    let id = document.create_embed("math", "x").unwrap();
    assert!(document.attach_embed(0, &id).is_err());
    document.edit(1, 0, "\u{FFFC}").unwrap();
    assert!(document.attach_embed(1, "missing").is_err());
    assert!(document.attach_embed(9, &id).is_err());
    document.attach_embed(1, &id).unwrap();
    assert_eq!(document.embeds()[0].position, 1);
}

#[test]
fn a_changed_source_applies_only_the_difference() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let document = library.create().unwrap();
    let id = document.create_embed("math", "a😀b").unwrap();
    assert!(document.set_embed_source(&id, "a😃b").unwrap());
    assert_eq!(document.embed_source(&id).as_deref(), Some("a😃b"));
    assert!(!document.set_embed_source(&id, "a😃b").unwrap());
    assert!(document.set_embed_source("missing", "x").is_err());
}

#[test]
fn an_asset_stays_after_a_reload_and_has_one_copy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let first = library.add_asset(b"picture").unwrap();
    assert_eq!(library.add_asset(b"picture").unwrap(), first);
    drop(library);
    let reopened = Library::open(&path).unwrap();
    assert_eq!(reopened.asset(&first).unwrap().as_deref(), Some(&b"picture"[..]));
    assert_eq!(reopened.asset("missing").unwrap(), None);
    assert!(reopened.add_asset(&vec![0; write_store::assets::MAX_ASSET_BYTES + 1]).is_err());
}

#[test]
fn trash_hides_a_document_and_restore_returns_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "Keep this").unwrap();
    let id = document.id();

    library.move_to_trash(&id).unwrap();
    assert!(library.list().unwrap().is_empty());
    assert_eq!(library.list_trash().unwrap()[0].title, "Keep this");
    assert_ne!(document.deleted_ms(), 0);

    drop(document);
    drop(library);
    let reopened = Library::open(&path).unwrap();
    assert!(reopened.list().unwrap().is_empty());
    reopened.restore_document(&id).unwrap();
    assert_eq!(reopened.list().unwrap()[0].id, id);
    assert!(reopened.list_trash().unwrap().is_empty());
    assert_eq!(reopened.open_document(&id).unwrap().text(), "Keep this");
}

#[test]
fn purge_removes_the_text_from_the_database_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("library.sqlite3");
    let library = Library::open(&path).unwrap();
    let document = library.create().unwrap();
    document.edit(0, 0, "private words").unwrap();
    let id = document.id();
    let holds_text = || {
        ["library.sqlite3", "library.sqlite3-wal"].iter().any(|name| {
            let bytes = std::fs::read(directory.path().join(name)).unwrap_or_default();
            bytes.windows(13).any(|window| window == b"private words")
        })
    };
    assert!(holds_text());
    library.purge_document(&id).unwrap();
    assert!(!holds_text());

    assert!(library.list().unwrap().is_empty());
    assert!(library.list_trash().unwrap().is_empty());
    assert!(library.open_document(&id).is_err());
    document.edit(0, 0, "late edit").unwrap();
    assert!(library.list().unwrap().is_empty());
}

#[test]
fn only_old_trash_expires() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let old = library.create().unwrap();
    let recent = library.create().unwrap();
    let kept = library.create().unwrap();
    library.move_to_trash(&old.id()).unwrap();
    let deleted = old.deleted_ms();
    std::thread::sleep(std::time::Duration::from_millis(5));
    library.move_to_trash(&recent.id()).unwrap();

    let keep = write_store::deletion::TRASH_KEEP_MS;
    assert!(library.purge_expired_at(deleted + keep - 1).unwrap().is_empty());
    assert_eq!(library.purge_expired_at(deleted + keep).unwrap(), vec![old.id()]);
    assert_eq!(library.list_trash().unwrap().len(), 1);
    assert_eq!(library.list().unwrap()[0].id, kept.id());
}

#[test]
fn purge_removes_only_images_that_no_other_document_uses() {
    let directory = tempfile::tempdir().unwrap();
    let library = Library::open(&directory.path().join("library.sqlite3")).unwrap();
    let shared = library.add_asset(b"shared").unwrap();
    let alone = library.add_asset(b"alone").unwrap();
    let first = library.create().unwrap();
    first.insert_embed(0, "image", &shared).unwrap();
    first.insert_embed(1, "image", &alone).unwrap();
    let second = library.create().unwrap();
    second.insert_embed(0, "image", &shared).unwrap();

    library.purge_document(&first.id()).unwrap();
    assert!(library.asset(&shared).unwrap().is_some());
    assert_eq!(library.asset(&alone).unwrap(), None);

    assert_eq!(library.add_asset(b"alone").unwrap(), alone);
    assert!(library.asset(&alone).unwrap().is_some());
}

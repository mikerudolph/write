use std::env;
use std::path::Path;
use write_store::sync::LocalFolder;
use write_store::Library;

fn main() {
    let arguments: Vec<String> = env::args().collect();
    let library = Library::open(Path::new(&arguments[1])).expect("Open the library");
    let folder = LocalFolder::new(Path::new(&arguments[2]));
    if arguments[3] == "show" {
        let report = library.sync(&folder).expect("Sync");
        assert_eq!(report.failure, None);
    }
    match arguments[3].as_str() {
        "add" => {
            let document = library.create().expect("Create a document");
            document.edit(0, 0, &arguments[4]).expect("Edit");
        }
        "add-math" => {
            let document = library.create().expect("Create a document");
            document.edit(0, 0, "\nMath from the Mac").expect("Edit");
            document.insert_embed(0, "math", &arguments[4]).expect("Insert");
        }
        "add-image" => {
            let document = library.create().expect("Create a document");
            document.edit(0, 0, "\nImage from the Mac").expect("Edit");
            let bytes = std::fs::read(&arguments[4]).expect("Read the image");
            let hash = library.add_asset(&bytes).expect("Add the image");
            document.insert_embed(0, "image", &hash).expect("Insert");
        }
        "trash" => {
            let id = library
                .list()
                .expect("List")
                .into_iter()
                .find(|item| item.title == arguments[4])
                .expect("Find the document")
                .id;
            library.move_to_trash(&id).expect("Move to Trash");
        }
        "trash-list" => {
            library.sync(&folder).expect("Sync");
            for item in library.list_trash().expect("List") {
                println!("trash {}", item.title);
            }
        }
        "append" => {
            let id = library.list().expect("List")[0].id.clone();
            let document = library.open_document(&id).expect("Open");
            let length = document.text().encode_utf16().count() as u64;
            document.edit(length, 0, &arguments[4]).expect("Edit");
        }
        "show" => {
            for item in library.list().expect("List") {
                let document = library.open_document(&item.id).expect("Open");
                println!("{}", document.text().replace('\n', " | "));
                println!("blocks={}", document.blocks().len());
                for embed in document.embeds() {
                    println!("embed {} {}", embed.kind, embed.source);
                }
            }
        }
        other => panic!("Unknown command: {other}"),
    }
    let report = library.sync(&folder).expect("Sync");
    assert_eq!(report.failure, None);
    println!("synced pushed={} pulled={}", report.pushed, report.pulled);
}

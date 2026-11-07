use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tempfile::tempdir;
use write_store::sync::{frame, parse_folder_path_for_test, unframe, FolderBackend, LocalFolder};
use write_store::Library;

struct Device {
    library: Library,
    folder: LocalFolder,
    root: PathBuf,
}

impl Device {
    fn new(directory: &Path, name: &str, root: &Path) -> Self {
        fs::create_dir_all(directory.join(name)).unwrap();
        Device {
            library: Library::open(&directory.join(name).join("library.sqlite3")).unwrap(),
            folder: LocalFolder::new(root),
            root: root.to_path_buf(),
        }
    }

    fn sync(&self) -> write_store::sync::SyncReport {
        let report = self.library.sync(&self.folder).unwrap();
        assert_eq!(report.failure, None);
        report
    }

    fn texts(&self) -> BTreeMap<String, String> {
        self.library
            .list()
            .unwrap()
            .into_iter()
            .map(|item| {
                let text = self.library.open_document(&item.id).unwrap().text();
                (item.id, text)
            })
            .collect()
    }
}

#[test]
fn a_document_reaches_another_device() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "# Notes\nHello 東京 😀").unwrap();
    let id = document.id();
    assert_eq!(first.sync().pushed, 1);
    let report = second.sync();
    assert_eq!(report.pulled, 1);
    assert_eq!(report.changed, [id.clone()]);
    assert_eq!(second.library.open_document(&id).unwrap().text(), "# Notes\nHello 東京 😀");
    assert_eq!(second.library.list().unwrap()[0].title, "Notes");
}

#[test]
fn an_open_document_receives_remote_edits() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "base").unwrap();
    first.sync();
    second.sync();
    let open = second.library.open_document(&document.id()).unwrap();
    document.edit(4, 0, " more").unwrap();
    first.sync();
    second.sync();
    assert_eq!(open.text(), "base more");
}

#[test]
fn concurrent_edits_merge_and_keep_all_text() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let left = first.library.create().unwrap();
    left.edit(0, 0, "one\ntwo").unwrap();
    first.sync();
    second.sync();
    let right = second.library.open_document(&left.id()).unwrap();
    left.edit(3, 0, " left").unwrap();
    right.edit(7, 0, " right").unwrap();
    first.sync();
    second.sync();
    first.sync();
    assert_eq!(left.text(), "one left\ntwo right");
    assert_eq!(right.text(), left.text());
}

#[test]
fn local_edits_made_between_syncs_are_not_lost() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let left = first.library.create().unwrap();
    left.edit(0, 0, "a").unwrap();
    first.sync();
    second.sync();
    let right = second.library.open_document(&left.id()).unwrap();
    right.edit(1, 0, "b").unwrap();
    second.sync();
    left.edit(1, 0, "c").unwrap();
    first.sync();
    left.edit(left.text().chars().count() as u64, 0, "d").unwrap();
    first.sync();
    second.sync();
    assert_eq!(right.text(), left.text());
    assert_eq!(left.text().len(), 4);
}

#[test]
fn a_new_device_rebuilds_the_library() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    for text in ["first document", "second document"] {
        first.library.create().unwrap().edit(0, 0, text).unwrap();
    }
    first.sync();
    let second = Device::new(directory.path(), "second", &shared);
    second.sync();
    assert_eq!(first.texts(), second.texts());
}

#[test]
fn a_new_device_keeps_all_text_after_a_reload() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    let id = document.id();
    document.edit(0, 0, "a").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&id).unwrap();
    for round in 0..6 {
        let left = document.text().chars().count() as u64;
        document.edit(left, 0, "b").unwrap();
        first.sync();
        second.sync();
        let right = copy.text().chars().count() as u64;
        copy.edit(right, 0, "c").unwrap();
        second.sync();
        first.sync();
        assert_eq!(document.text(), copy.text(), "round {round}");
    }
    let expected = document.text();
    let third = Device::new(directory.path(), "third", &shared);
    third.sync();
    assert_eq!(third.library.open_document(&id).unwrap().text(), expected);
    let reopened = Library::open(&directory.path().join("third").join("library.sqlite3")).unwrap();
    assert_eq!(reopened.open_document(&id).unwrap().text(), expected);
}

#[test]
fn block_ids_reach_another_device() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "one\ntwo\nthree").unwrap();
    let embed = document.insert_embed(4, "math", "x^2").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&document.id()).unwrap();
    assert_eq!(copy.blocks(), document.blocks());
    assert_eq!(copy.embed_source(&embed).as_deref(), Some("x^2"));
}

struct Flaky {
    inner: LocalFolder,
    online: AtomicBool,
}

impl FolderBackend for Flaky {
    fn list(&self) -> Result<Vec<String>, String> {
        self.inner.list()
    }
    fn read(&self, path: &str) -> Result<Vec<u8>, String> {
        self.inner.read(path)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> Result<(), String> {
        if self.online.load(Ordering::Relaxed) {
            self.inner.write(path, bytes)
        } else {
            Err("The folder is not available".to_string())
        }
    }
    fn delete(&self, path: &str) -> Result<(), String> {
        self.inner.delete(path)
    }
}

#[test]
fn a_failed_write_stays_in_the_outbox_and_survives_a_restart() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    fs::create_dir_all(&shared).unwrap();
    LocalFolder::new(&shared).write("VERSION", b"1").unwrap();
    let path = directory.path().join("library.sqlite3");
    let backend = Flaky { inner: LocalFolder::new(&shared), online: AtomicBool::new(false) };
    let id = {
        let library = Library::open(&path).unwrap();
        let document = library.create().unwrap();
        document.edit(0, 0, "kept").unwrap();
        let report = library.sync(&backend).unwrap();
        assert!(report.failure.is_some());
        document.id()
    };
    backend.online.store(true, Ordering::Relaxed);
    let library = Library::open(&path).unwrap();
    assert!(library.sync(&backend).unwrap().pushed >= 1);
    let second = Device::new(directory.path(), "second", &shared);
    second.sync();
    assert_eq!(second.library.open_document(&id).unwrap().text(), "kept");
}

#[test]
fn a_truncated_file_waits_and_a_conflicted_copy_is_ignored() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "text").unwrap();
    first.sync();
    let path = first
        .folder
        .list()
        .unwrap()
        .into_iter()
        .find(|path| path.ends_with(".update"))
        .unwrap();
    let complete = fs::read(shared.join(&path)).unwrap();
    fs::write(shared.join(&path), &complete[..complete.len() / 2]).unwrap();
    let conflicted = path.replace("0000000001.update", "0000000001 (conflicted copy).update");
    fs::write(shared.join(conflicted), &complete).unwrap();
    let report = second.sync();
    assert_eq!((report.pulled, report.waiting), (0, 1));
    fs::write(shared.join(&path), &complete).unwrap();
    assert_eq!(second.sync().pulled, 1);
    assert_eq!(second.library.open_document(&document.id()).unwrap().text(), "text");
}

#[test]
fn a_folder_from_a_newer_version_is_refused() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    fs::create_dir_all(&shared).unwrap();
    fs::write(shared.join("VERSION"), "99").unwrap();
    let device = Device::new(directory.path(), "first", &shared);
    let report = device.library.sync(&device.folder).unwrap();
    assert!(report.failure.is_some());
}

#[test]
fn snapshots_limit_the_file_count_and_rebuild_a_new_device() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let document = first.library.create().unwrap();
    for round in 0..250u64 {
        document.edit(round, 0, "x").unwrap();
        first.sync();
    }
    let count = first.folder.list().unwrap().len();
    assert!(count < 160, "The folder has {count} files");
    let second = Device::new(directory.path(), "second", &first.root);
    second.sync();
    assert_eq!(second.library.open_document(&document.id()).unwrap().text(), "x".repeat(250));
}

#[test]
fn a_device_never_writes_to_another_device_directory() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "one").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&document.id()).unwrap();
    copy.edit(3, 0, " two").unwrap();
    second.sync();
    let owners: std::collections::HashSet<String> = first
        .folder
        .list()
        .unwrap()
        .iter()
        .filter_map(|path| parse_folder_path_for_test(path))
        .collect();
    let first_id = first.library.device_id().unwrap();
    let second_id = second.library.device_id().unwrap();
    assert_eq!(owners, [first_id, second_id].into_iter().collect());
}

fn deliver(source: &Path, target: &Path, random: &mut u64, percent: u64) {
    let files = LocalFolder::new(source).list().unwrap();
    for path in files {
        *random = random.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        if (*random >> 33) % 100 >= percent {
            continue;
        }
        let bytes = fs::read(source.join(&path)).unwrap();
        if path.ends_with(".update") && unframe(&bytes).is_none() {
            continue;
        }
        let destination = target.join(&path);
        if fs::read(&destination).is_ok_and(|current| unframe(&current).is_some()) {
            continue;
        }
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        let truncate = (*random >> 20) % 10 == 0;
        let length = if truncate { bytes.len() / 2 } else { bytes.len() };
        fs::write(&destination, &bytes[..length]).unwrap();
    }
}

#[test]
fn three_devices_converge_with_bad_delivery() {
    for seed in 0..60u64 {
        let directory = tempdir().unwrap();
        let roots: Vec<PathBuf> = (0..3).map(|index| directory.path().join(format!("replica-{index}"))).collect();
        let devices: Vec<Device> = roots
            .iter()
            .enumerate()
            .map(|(index, root)| Device::new(directory.path(), &format!("device-{index}"), root))
            .collect();
        let origin = devices[0].library.create().unwrap();
        origin.edit(0, 0, "start\n").unwrap();
        let id = origin.id();
        devices[0].sync();
        for to in 1..3 {
            deliver(&roots[0], &roots[to], &mut 1, 100);
            for _ in 0..3 {
                deliver(&roots[0], &roots[to], &mut 5, 100);
            }
            let path = LocalFolder::new(&roots[0]).list().unwrap();
            for item in path {
                let destination = roots[to].join(&item);
                fs::create_dir_all(destination.parent().unwrap()).unwrap();
                fs::copy(roots[0].join(&item), destination).unwrap();
            }
            devices[to].sync();
        }
        let mut documents: Vec<_> = devices.iter().map(|device| device.library.open_document(&id).unwrap()).collect();
        let mut random = seed + 11;
        let mut added = String::new();
        for _ in 0..80 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let index = ((random >> 33) % 3) as usize;
            let document = &mut documents[index];
            let length = document.text().chars().count() as u64;
            let token = ["a", "b", "東", "\n"][((random >> 40) % 4) as usize];
            document.edit((random >> 24) % (length + 1), 0, token).unwrap();
            added.push_str(token);
            if (random >> 12) % 3 == 0 {
                devices[index].sync();
            }
            if (random >> 14) % 3 == 0 {
                let from = ((random >> 16) % 3) as usize;
                let to = ((random >> 18) % 3) as usize;
                if from != to {
                    deliver(&roots[from], &roots[to], &mut random, 60);
                    devices[to].sync();
                }
            }
        }
        for _ in 0..3 {
            for device in &devices {
                device.sync();
            }
            for from in 0..3 {
                for to in 0..3 {
                    if from == to {
                        continue;
                    }
                    for path in LocalFolder::new(&roots[from]).list().unwrap() {
                        let bytes = fs::read(roots[from].join(&path)).unwrap();
                        if path.ends_with("VERSION") || unframe(&bytes).is_some() {
                            let destination = roots[to].join(&path);
                            fs::create_dir_all(destination.parent().unwrap()).unwrap();
                            fs::write(destination, bytes).unwrap();
                        }
                    }
                }
            }
        }
        for device in &devices {
            device.sync();
        }
        let expected = documents[0].text();
        assert_eq!(documents[1].text(), expected, "seed {seed}");
        assert_eq!(documents[2].text(), expected, "seed {seed}");
        assert_eq!(expected.chars().count(), "start\n".chars().count() + added.chars().count(), "seed {seed}");
        let ids: Vec<String> = documents[0].blocks().into_iter().map(|block| block.id).collect();
        assert!(ids.iter().all(|id| !id.is_empty()), "seed {seed}");
    }
    let _ = (frame(b""), Arc::new(0));
}

#[test]
fn an_asset_reaches_another_device_once() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let hash = first.library.add_asset(b"picture").unwrap();
    first.sync();
    assert_eq!(second.sync().assets_received, 1);
    assert_eq!(second.library.asset(&hash).unwrap().as_deref(), Some(&b"picture"[..]));
    assert_eq!(second.sync().assets_received, 0);
    first.sync();
    assert_eq!(fs::read_dir(shared.join("assets")).unwrap().count(), 1);
}

#[test]
fn a_damaged_asset_waits_for_a_good_copy() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let hash = first.library.add_asset(b"picture").unwrap();
    first.sync();
    let device = fs::read_dir(shared.join("assets")).unwrap().next().unwrap().unwrap().path();
    let file = device.join(format!("{hash}.asset"));
    let good = fs::read(&file).unwrap();

    fs::write(&file, &good[..good.len() - 2]).unwrap();
    let report = second.sync();
    assert_eq!((report.assets_received, report.waiting), (0, 1));
    assert_eq!(second.library.asset(&hash).unwrap(), None);

    fs::write(&file, frame(b"other")).unwrap();
    assert_eq!(second.sync().waiting, 1);
    assert_eq!(second.library.asset(&hash).unwrap(), None);

    fs::write(&file, &good).unwrap();
    assert_eq!(second.sync().assets_received, 1);
}

#[test]
fn a_new_location_receives_all_assets() {
    let directory = tempdir().unwrap();
    let first_location = directory.path().join("one");
    let second_location = directory.path().join("two");
    let device = Device::new(directory.path(), "device", &first_location);
    device.library.add_asset(b"picture").unwrap();
    device.sync();
    let moved = LocalFolder::new(&second_location);
    device.library.sync(&moved).unwrap();
    assert!(second_location.join("assets").exists());
}

#[test]
fn concurrent_source_edits_merge_and_keep_the_block() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "before\n\nafter").unwrap();
    let id = document.insert_embed(7, "math", "b").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&document.id()).unwrap();

    document.set_embed_source(&id, "ab").unwrap();
    copy.set_embed_source(&id, "bc").unwrap();
    document.edit(0, 0, "Start ").unwrap();
    first.sync();
    second.sync();
    first.sync();

    assert_eq!(document.embed_source(&id).as_deref(), Some("abc"));
    assert_eq!(copy.embed_source(&id).as_deref(), Some("abc"));
    assert_eq!(document.embeds(), copy.embeds());
    assert_eq!(document.embeds()[0].position, 13);
}

#[test]
fn a_block_deleted_on_one_device_stays_deleted_after_a_source_edit_on_another() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "a\n\nb").unwrap();
    let id = document.insert_embed(2, "math", "x").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&document.id()).unwrap();

    document.edit(2, 1, "").unwrap();
    copy.set_embed_source(&id, "xy").unwrap();
    first.sync();
    second.sync();
    first.sync();

    assert!(document.embeds().is_empty());
    assert!(copy.embeds().is_empty());
    assert_eq!(document.text(), copy.text());
}

#[test]
fn trash_and_restore_reach_another_device() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "text").unwrap();
    first.sync();
    second.sync();
    assert_eq!(second.library.list().unwrap().len(), 1);

    first.library.move_to_trash(&document.id()).unwrap();
    first.sync();
    second.sync();
    assert!(second.library.list().unwrap().is_empty());
    assert_eq!(second.library.list_trash().unwrap().len(), 1);

    second.library.restore_document(&document.id()).unwrap();
    second.sync();
    first.sync();
    assert_eq!(first.library.list().unwrap().len(), 1);
    assert!(first.library.list_trash().unwrap().is_empty());
}

#[test]
fn an_edit_during_trash_stays_in_the_document() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "text").unwrap();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&document.id()).unwrap();

    first.library.move_to_trash(&document.id()).unwrap();
    copy.edit(4, 0, " and more").unwrap();
    first.sync();
    second.sync();
    first.sync();

    assert!(first.library.list().unwrap().is_empty());
    assert!(second.library.list().unwrap().is_empty());
    first.library.restore_document(&document.id()).unwrap();
    assert_eq!(document.text(), "text and more");
}

#[test]
fn a_new_device_sees_a_trashed_document_in_trash() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "text").unwrap();
    first.library.move_to_trash(&document.id()).unwrap();
    first.sync();
    let fresh = Device::new(directory.path(), "fresh", &shared);
    fresh.sync();
    assert!(fresh.library.list().unwrap().is_empty());
    assert_eq!(fresh.library.list_trash().unwrap().len(), 1);
}

fn document_files(root: &Path, id: &str) -> usize {
    fn count(path: &Path) -> usize {
        match fs::read_dir(path) {
            Ok(entries) => entries
                .map(|entry| {
                    let path = entry.unwrap().path();
                    if path.is_dir() { count(&path) } else { 1 }
                })
                .sum(),
            Err(_) => 0,
        }
    }
    count(&root.join("documents").join(id))
}

#[test]
fn a_purge_removes_the_document_from_each_device_and_the_folder() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "first").unwrap();
    let id = document.id();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&id).unwrap();
    copy.edit(0, 0, "second ").unwrap();
    second.sync();
    first.sync();
    assert!(document_files(&shared, &id) >= 2);

    first.library.purge_document(&id).unwrap();
    first.sync();
    let report = second.sync();
    assert_eq!(report.removed, vec![id.clone()]);
    assert!(second.library.list().unwrap().is_empty());
    assert!(second.library.list_trash().unwrap().is_empty());
    second.sync();
    first.sync();
    assert_eq!(document_files(&shared, &id), 0);
    assert!(first.library.list().unwrap().is_empty());

    let fresh = Device::new(directory.path(), "fresh", &shared);
    fresh.sync();
    assert!(fresh.library.list().unwrap().is_empty());
    assert!(fresh.library.list_trash().unwrap().is_empty());
}

#[test]
fn an_offline_device_cannot_bring_back_a_purged_document() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let document = first.library.create().unwrap();
    document.edit(0, 0, "first").unwrap();
    let id = document.id();
    first.sync();
    second.sync();
    let copy = second.library.open_document(&id).unwrap();

    first.library.purge_document(&id).unwrap();
    first.sync();
    copy.edit(0, 0, "offline ").unwrap();
    second.sync();
    first.sync();
    second.sync();

    assert!(first.library.list().unwrap().is_empty());
    assert!(second.library.list().unwrap().is_empty());
    assert_eq!(document_files(&shared, &id), 0);
}

#[test]
fn a_purged_image_leaves_the_folder_and_stays_away() {
    let directory = tempdir().unwrap();
    let shared = directory.path().join("shared");
    let first = Device::new(directory.path(), "first", &shared);
    let second = Device::new(directory.path(), "second", &shared);
    let hash = first.library.add_asset(b"picture").unwrap();
    let document = first.library.create().unwrap();
    document.insert_embed(0, "image", &hash).unwrap();
    first.sync();
    second.sync();
    first.sync();

    first.library.purge_document(&document.id()).unwrap();
    first.sync();
    second.sync();
    first.sync();
    second.sync();

    assert_eq!(first.library.asset(&hash).unwrap(), None);
    assert_eq!(second.library.asset(&hash).unwrap(), None);
    let remaining = LocalFolder::new(&shared).list().unwrap();
    assert!(!remaining.iter().any(|path| path.contains(&hash)), "{remaining:?}");
}

#[test]
fn a_tombstone_reaches_a_new_location() {
    let directory = tempdir().unwrap();
    let one = directory.path().join("one");
    let two = directory.path().join("two");
    let device = Device::new(directory.path(), "device", &one);
    let document = device.library.create().unwrap();
    device.library.purge_document(&document.id()).unwrap();
    device.library.sync(&LocalFolder::new(&two)).unwrap();
    assert!(two.join("purged").exists());
}

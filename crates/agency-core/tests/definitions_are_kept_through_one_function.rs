//! A definition is kept through `push_devices::keep_org`, which also drops the push registrations of a device
//! the definition no longer has. A source that calls the store's `save_org` itself would leave them behind.
use std::path::{Path, PathBuf};

const THE_ONE: &str = "push_devices.rs";

fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn only_the_one_function_saves_a_definition() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = vec![];
    for member in std::fs::read_dir(&crates).unwrap().flatten() {
        rust_files(&member.path().join("src"), &mut files);
    }
    assert!(files.len() > 50, "the crates' sources were found");
    let mut saving = vec![];
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let is_the_one = file.file_name().is_some_and(|name| name == THE_ONE);
        for (number, line) in text.lines().enumerate() {
            if line.contains(".save_org(") && !is_the_one {
                saving.push(format!("{}:{}", file.display(), number + 1));
            }
        }
    }
    assert!(
        saving.is_empty(),
        "keep these through push_devices::keep_org: {saving:?}"
    );
}

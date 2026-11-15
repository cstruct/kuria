//! Embeds every file of the bundled packages (`packages/preview/<name>/<version>/`)
//! as `PACKAGE_FILES`, `(name, version, path, bytes)`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn files(dir: &Path, found: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .expect("a readable package directory")
        .map(|e| e.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("packages/preview");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut found = Vec::new();
    files(&root, &mut found);
    let mut out = String::from(
        "/// Every file of the bundled packages: name, version, path, bytes.\npub(crate) static PACKAGE_FILES: &[(&str, &str, &str, &[u8])] = &[\n",
    );
    for path in found {
        let relative = path.strip_prefix(&root).expect("under the packages");
        let mut parts = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned());
        let (Some(name), Some(version)) = (parts.next(), parts.next()) else {
            continue;
        };
        let inner: Vec<String> = parts.collect();
        if inner.is_empty() {
            continue;
        }
        let _ = writeln!(
            out,
            "    ({name:?}, {version:?}, {:?}, include_bytes!({:?})),",
            inner.join("/"),
            path.display().to_string()
        );
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("packages.rs");
    std::fs::write(dest, out).expect("written");
}

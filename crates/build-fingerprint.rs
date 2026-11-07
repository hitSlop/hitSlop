//! Build identities include local path dependencies, whose sources Cargo.lock omits.
use std::{
    fs,
    path::{Path, PathBuf},
};

fn visit(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read implementation inputs") {
        let entry = entry.expect("implementation input");
        if entry.file_name() == ".DS_Store" {
            continue;
        }
        let path = entry.path();
        if path.is_dir() { visit(&path, files) } else { files.push(path) }
    }
}

pub fn emit(variable: &str, packages: &[&str], extra_directories: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = root.parent().and_then(Path::parent).expect("crate lives in workspace");
    let mut files = vec![
        workspace.join("Cargo.toml"),
        workspace.join("Cargo.lock"),
        workspace.join("rust-toolchain.toml"),
        workspace.join("crates/build-fingerprint.rs"),
    ];
    for name in packages {
        let package = workspace.join("crates").join(name);
        files.push(package.join("Cargo.toml"));
        if package.join("build.rs").exists() {
            files.push(package.join("build.rs"));
        }
        let source = package.join("src");
        println!("cargo:rerun-if-changed={}", source.display());
        visit(&source, &mut files);
    }
    for name in extra_directories {
        let directory = workspace.join(name);
        println!("cargo:rerun-if-changed={}", directory.display());
        visit(&directory, &mut files);
    }
    files.sort();
    files.dedup();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file.strip_prefix(workspace).expect("workspace input").to_string_lossy();
        for byte in name.bytes().chain([0]).chain(fs::read(&file).expect("implementation bytes")) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    println!("cargo:rustc-env={variable}={hash:016x}");
}

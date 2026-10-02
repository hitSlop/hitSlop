//! Sets HITSLOP_CORE_BUILD_ID over core sources, compiled schemas and locked dependencies.
use std::{fs, path::Path};
fn visit(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() { visit(&path, files) } else { files.push(path) }
    }
}
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![root.join("Cargo.toml"), root.join("build.rs"), root.join("../../Cargo.lock")];
    // Native validators compile these files into the core. Include them even in WASM
    // builds so both adapters identify the same rules, regardless of enabled features.
    for name in ["manifest", "package-manifest", "socket-request", "socket-reply", "socket-discovery", "page-request"] {
        files.push(root.join(format!("../../packages/schema/generated/{name}.schema.json")));
    }
    visit(&root.join("src"), &mut files);
    files.sort();
    let mut hash: u64 = 0xcbf29ce484222325;
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file.strip_prefix(root).unwrap_or(file).to_string_lossy().into_owned();
        for byte in name.bytes().chain([0]).chain(fs::read(file).unwrap()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rustc-env=HITSLOP_CORE_BUILD_ID={hash:016x}");
}

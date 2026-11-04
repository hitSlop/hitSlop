fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/file/relocate.m");
        cc::Build::new().file("src/file/relocate.m").flag("-fobjc-arc").compile("pop_relocation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
}

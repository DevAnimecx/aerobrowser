fn main() {
    println!("cargo:rerun-if-env-changed=DELTA_BUNDLED_MOLI_ARCHIVE");
    if std::env::var_os("CARGO_FEATURE_BUNDLED_MOLI").is_some() {
        let source = std::env::var_os("DELTA_BUNDLED_MOLI_ARCHIVE").expect(
            "bundled-moli needs DELTA_BUNDLED_MOLI_ARCHIVE pointing to the pinned release archive",
        );
        println!(
            "cargo:rerun-if-changed={}",
            std::path::Path::new(&source).display()
        );
        let out =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("moli.tar.gz");
        std::fs::copy(source, out).expect("copy pinned Moli archive");
    }
}

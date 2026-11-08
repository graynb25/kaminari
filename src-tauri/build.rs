fn main() {
    println!("cargo:rerun-if-changed=../VERSION");

    let version = std::fs::read_to_string("../VERSION")
        .expect("VERSION file is missing")
        .trim()
        .to_string();
    let cargo_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();

    if version != cargo_version {
        panic!("VERSION is {version} but Cargo.toml is {cargo_version}. Run `pnpm version:sync`.");
    }

    tauri_build::build()
}

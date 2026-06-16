fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for entry in std::fs::read_dir("web").unwrap().flatten() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
    }
}

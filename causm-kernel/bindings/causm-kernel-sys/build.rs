use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let dist_dir = PathBuf::from(&manifest_dir).join("../../dist");
    let canonical_dist = dist_dir.canonicalize().unwrap_or(dist_dir);

    println!(
        "cargo:rustc-link-search=native={}",
        canonical_dist.display()
    );
    println!("cargo:rustc-link-lib=static=causm_kernel");
    println!(
        "cargo:rerun-if-changed={}",
        canonical_dist.join("libcausm_kernel.a").display()
    );
}

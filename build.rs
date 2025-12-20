fn main() {
    // Explicitly link against libgit2 and libssh2
    // This works around issues where the -sys crates' build.rs output
    // isn't being properly processed by cargo
    println!("cargo:rustc-link-lib=git2");
    println!("cargo:rustc-link-lib=ssh2");
}

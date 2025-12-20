fn main() {
    // Force linking to system libgit2 and libssh2
    // This ensures the linker includes these libraries even when
    // libgit2-sys's build.rs has issues detecting them via pkg-config
    println!("cargo:rustc-link-lib=git2");
    println!("cargo:rustc-link-lib=ssh2");
}

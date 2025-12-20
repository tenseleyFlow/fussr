fn main() {
    // Explicitly link against libgit2, libssh2, and their dependencies
    // Use link-arg to append to the END of the linker command line
    // This ensures the libraries come AFTER the Rust code that needs them
    // Order matters: libgit2 depends on libssh2, both depend on zlib
    println!("cargo:rustc-link-arg=-lgit2");
    println!("cargo:rustc-link-arg=-lssh2");
    println!("cargo:rustc-link-arg=-lz");
}

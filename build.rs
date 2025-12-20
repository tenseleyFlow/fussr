fn main() {
    // Explicitly link against libgit2 and libssh2
    // Use link-arg to append to the END of the linker command line
    // This ensures the libraries come AFTER the Rust code that needs them
    println!("cargo:rustc-link-arg=-lgit2");
    println!("cargo:rustc-link-arg=-lssh2");
}

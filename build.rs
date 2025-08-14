use std::env;

fn main() {
    // If the "cuda" feature is enabled, set up link paths and libs without probing.
    if env::var("CARGO_FEATURE_CUDA").is_ok() {
        println!("cargo:rustc-link-search=/usr/local/cuda/lib64");
        println!("cargo:rustc-link-search=/opt/cuda/lib64");

        println!("cargo:rustc-link-lib=cublas");
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cublasLt");
        println!("cargo:rustc-link-lib=culibos");
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=nvrtc");
    }
}

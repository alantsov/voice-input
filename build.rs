use std::process::Command;

fn main() {
    // Check if CUDA is available by running nvidia-smi
    let cuda_available = Command::new("nvidia-smi").status().is_ok();

    if cuda_available {
        println!("cargo:rustc-cfg=feature=\"cuda_available\"");
        println!("cargo:warning=CUDA detected, enabling GPU acceleration");

        // Set a feature flag that we can use in Cargo.toml
        println!("cargo:rustc-cfg=cuda_available");

        // Add CUDA library paths
        println!("cargo:rustc-link-search=/usr/local/cuda/lib64");
        println!("cargo:rustc-link-search=/opt/cuda/lib64");

        // Link CUDA libraries
        println!("cargo:rustc-link-lib=cublas");
        println!("cargo:rustc-link-lib=cudart");
        println!("cargo:rustc-link-lib=cublasLt");
        println!("cargo:rustc-link-lib=culibos");
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rustc-link-lib=nvrtc");
    } else {
        println!("cargo:warning=CUDA not detected, disabling GPU acceleration");
    }
}

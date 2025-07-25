use std::process::Command;
use std::env;
use std::path::Path;

fn main() {
    // Check if the CUDA feature is enabled
    let cuda_feature_enabled = env::var("CARGO_FEATURE_CUDA").is_ok();

    if cuda_feature_enabled {
        println!("cargo:warning=CUDA feature is enabled");

        // Check if CUDA is available by running nvidia-smi
        let cuda_available = Command::new("nvidia-smi").status().is_ok();

        // Check for CUDA libraries
        let cuda_libs_available = check_cuda_libraries();

        if cuda_available && cuda_libs_available {
            println!("cargo:rustc-cfg=feature=\"cuda_available\"");
            println!("cargo:warning=CUDA detected and libraries found, enabling GPU acceleration");

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
            println!("cargo:warning=CUDA feature is enabled but CUDA is not available on this system");
            println!("cargo:warning=GPU acceleration will be disabled");

            if !cuda_available {
                println!("cargo:warning=nvidia-smi command failed or not found");
            }

            if !cuda_libs_available {
                println!("cargo:warning=CUDA libraries not found");
            }
        }
    } else {
        println!("cargo:warning=CUDA feature is not enabled, disabling GPU acceleration");
    }
}

fn check_cuda_libraries() -> bool {
    // Check common CUDA library paths
    let cuda_paths = [
        "/usr/local/cuda/lib64",
        "/opt/cuda/lib64",
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib/cuda/lib64",
    ];

    // Check for essential CUDA libraries
    let essential_libs = [
        "libcuda.so",
        "libcudart.so",
        "libcublas.so",
    ];

    for path in &cuda_paths {
        if Path::new(path).exists() {
            let mut all_found = true;

            for lib in &essential_libs {
                let lib_path = Path::new(path).join(lib);
                if !lib_path.exists() {
                    all_found = false;
                    break;
                }
            }

            if all_found {
                println!("cargo:warning=Found CUDA libraries in {}", path);
                return true;
            }
        }
    }

    // Try to check using ldconfig
    if let Ok(output) = Command::new("ldconfig").args(["-p"]).output() {
        let ldconfig_output = String::from_utf8_lossy(&output.stdout);
        let mut found_libs = 0;

        for lib in &essential_libs {
            if ldconfig_output.contains(lib) {
                found_libs += 1;
            }
        }

        if found_libs == essential_libs.len() {
            println!("cargo:warning=Found CUDA libraries in ldconfig cache");
            return true;
        }
    }

    false
}

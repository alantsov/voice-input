use std::fs::File;
use std::process;
use directories::ProjectDirs;
use fs2::FileExt;

/// Ensure only a single instance of the app is running by creating and locking a file.
/// Returns the opened lock file, which must be kept in scope for the duration of the program.
pub fn ensure_single_instance() -> File {
    // Implement single instance check
    let lock_file = if let Some(proj_dirs) = ProjectDirs::from("com", "voice-input", "voice-input") {
        let cache_dir = proj_dirs.cache_dir();
        std::fs::create_dir_all(cache_dir).unwrap_or_else(|e| {
            eprintln!("Failed to create cache directory: {}", e);
            process::exit(1);
        });
        let lock_path = cache_dir.join("voice-input.lock");
        match File::create(&lock_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to create lock file: {}", e);
                process::exit(1);
            }
        }
    } else {
        // Fallback to temp directory if ProjectDirs fails
        let lock_path = std::env::temp_dir().join("voice-input.lock");
        match File::create(&lock_path) {
            Ok(file) => file,
            Err(e) => {
                eprintln!("Failed to create lock file: {}", e);
                process::exit(1);
            }
        }
    };

    // Try to acquire an exclusive lock
    // The lock will be automatically released when the program exits
    // or when the returned file goes out of scope
    if let Err(_) = lock_file.try_lock_exclusive() {
        eprintln!("Another instance of Voice Input is already running.");
        process::exit(0);
    }

    lock_file
}
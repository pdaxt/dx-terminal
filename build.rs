use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn main() {
    // Auto-install git hooks on build so agents always have the CI gate.
    // Only runs when hooks/ dir exists and we're in a git repo.
    let hooks_src = Path::new("hooks");
    let hooks_dst = Path::new(".git/hooks");

    if hooks_src.is_dir() && hooks_dst.is_dir() {
        if let Ok(entries) = fs::read_dir(hooks_src) {
            for entry in entries.flatten() {
                let src = entry.path();
                if src.is_file() {
                    let name = entry.file_name();
                    let dst = hooks_dst.join(&name);
                    if fs::copy(&src, &dst).is_ok() {
                        // Make executable
                        if let Ok(meta) = fs::metadata(&dst) {
                            let mut perms = meta.permissions();
                            perms.set_mode(0o755);
                            let _ = fs::set_permissions(&dst, perms);
                        }
                    }
                }
            }
        }
    }

    // Only re-run if hooks change
    println!("cargo:rerun-if-changed=hooks/");
}

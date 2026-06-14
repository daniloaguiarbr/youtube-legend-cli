fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Ok(head) = std::fs::read_to_string(".git/HEAD") {
        let head = head.trim();
        if let Some(rest) = head.strip_prefix("ref: refs/heads/") {
            println!("cargo:rustc-env=GIT_BRANCH={rest}");
        }
        if let Ok(sha) = std::fs::read_to_string(format!(".git/{head}")) {
            let sha = sha.trim();
            if !sha.is_empty() {
                println!("cargo:rustc-env=GIT_SHA={sha}");
            }
        }
    }
    println!(
        "cargo:rustc-env=BUILD_TIMESTAMP={}",
        chrono::Utc::now().to_rfc3339()
    );
}

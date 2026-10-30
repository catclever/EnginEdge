use anyhow::{anyhow, Result};
use std::env;
use std::path::{Path, PathBuf};
use regex::Regex;
use once_cell::sync::Lazy;

static REDACT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(ghp_[a-zA-Z0-9]{36}|gho_[a-zA-Z0-9]{36}|sk-[a-zA-Z0-9]{48}|AKIA[0-9A-Z]{16})\b").unwrap()
});

pub fn validate_path<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let path = path.as_ref();
    let cwd = env::current_dir()?;

    if path.is_absolute() && !path.starts_with(&cwd) {
        return Err(anyhow!("Access denied: Absolute path outside of workspace"));
    }

    for comp in path.components() {
        if matches!(comp, std::path::Component::ParentDir) {
            return Err(anyhow!("Access denied: Path traversal detected"));
        }
    }

    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    // To prevent symlink escapes even for non-existent files:
    // 1. We must verify that the path does not resolve outside the workspace.
    // 2. Since `canonicalize()` fails on non-existent files, and checking parent's `canonicalize`
    //    can still allow a symlink attack if the file itself is a broken symlink that an attacker
    //    is trying to create/overwrite, we will just canonicalize the parent but also
    //    ensure that the file name itself doesn't cause issues.
    //    Actually, rust `fs::canonicalize` resolves symlinks. But if the file is a symlink and exists,
    //    canonicalize will resolve it. If it does NOT exist, there is no symlink yet.
    //    Wait, if the file is a symlink pointing outside, but the target doesn't exist? Then `exists()` is false!
    //    So if it's a broken symlink, `canonicalize()` would fail, and we'd fallback to parent.
    //    To fix this, we should use `std::fs::symlink_metadata` to check if it's a symlink,
    //    even if it's broken.

    if let Ok(metadata) = std::fs::symlink_metadata(&abs_path) {
        if metadata.is_symlink() {
            // Attempt to resolve it. If it fails, reject it for safety.
            match abs_path.canonicalize() {
                Ok(resolved) => {
                    if !resolved.starts_with(&cwd) {
                        return Err(anyhow!("Access denied: Path resolves outside of workspace"));
                    }
                }
                Err(_) => return Err(anyhow!("Access denied: Broken symlink detected")),
            }
        }
    }

    let resolved_path = if abs_path.exists() {
        abs_path.canonicalize()?
    } else {
        if let Some(parent) = abs_path.parent() {
            if parent.exists() {
                parent.canonicalize()?.join(abs_path.file_name().unwrap())
            } else {
                // If parent doesn't exist, we reject it because we can't safely resolve it
                return Err(anyhow!("Access denied: Parent directory does not exist"));
            }
        } else {
            abs_path.clone()
        }
    };

    if !resolved_path.starts_with(&cwd) {
         return Err(anyhow!("Access denied: Path resolves outside of workspace"));
    }

    let file_name = resolved_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let path_str = resolved_path.to_string_lossy();

    if file_name.starts_with(".env") ||
       file_name.starts_with("id_rsa") ||
       file_name.starts_with("id_ed25519") ||
       file_name.ends_with(".pem") ||
       file_name.ends_with(".key") {
        return Err(anyhow!("Access denied: Sensitive file"));
    }

    if path_str.contains("/.git/credentials") ||
       path_str.contains("/.aws/") ||
       path_str.contains("/.ssh/") ||
       path_str.ends_with("/.git/credentials") ||
       path_str.ends_with("/.aws") ||
       path_str.ends_with("/.ssh") {
        return Err(anyhow!("Access denied: Sensitive directory or file"));
    }

    Ok(resolved_path)
}

pub fn redact_secrets(input: &str) -> String {
    REDACT_REGEX.replace_all(input, "[REDACTED_SECRET]").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_secrets() {
        let input = "Here is my token: ghp_123456789012345678901234567890123456, keep it safe!";
        let redacted = redact_secrets(input);
        assert_eq!(redacted, "Here is my token: [REDACTED_SECRET], keep it safe!");
    }

    #[test]
    fn test_validate_path_traversal() {
        let current_dir = env::current_dir().unwrap();

        let path = current_dir.join("../some_file.txt");
        assert!(validate_path(path).is_err());

        let path_abs = Path::new("/etc/passwd");
        assert!(validate_path(path_abs).is_err());
    }

    #[test]
    fn test_validate_path_sensitive() {
        let current_dir = env::current_dir().unwrap();
        let file_env = current_dir.join(".env");
        assert!(validate_path(&file_env).is_err());
    }

    #[test]
    fn test_validate_path_allowed() {
        let current_dir = env::current_dir().unwrap();
        let file_ok = current_dir.join("src/main.rs");
        assert!(validate_path(&file_ok).is_ok());
    }
}

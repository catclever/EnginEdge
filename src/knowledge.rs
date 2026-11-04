use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Finds the workspace root directory by looking for a `.git` folder,
/// falling back to the current directory if not found.
fn find_workspace_root() -> PathBuf {
    let mut current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    loop {
        if current_dir.join(".git").exists() {
            return current_dir;
        }
        if !current_dir.pop() {
            // Reached the root of the filesystem
            break;
        }
    }

    env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Resolves the knowledge base metadata directory.
pub fn resolve_metadata_dir() -> Option<PathBuf> {
    if let Ok(dir) = env::var("ENGINEDGE_META_DIR") {
        return Some(PathBuf::from(dir));
    }
    if let Ok(dir) = env::var("AGY_PROJECT_META_DIR") {
        return Some(PathBuf::from(dir));
    }

    let root = find_workspace_root();
    let project_name = root.file_name()?.to_string_lossy().into_owned();

    let home_dir = env::var("HOME").ok().or_else(|| env::var("USERPROFILE").ok())?;

    let path = PathBuf::from(home_dir)
        .join(".gemini")
        .join("Projects")
        .join(&project_name);

    Some(path)
}

/// Checks if the knowledge base exists and has a `kb/` folder.
fn get_kb_dir() -> Option<PathBuf> {
    let meta_dir = resolve_metadata_dir()?;
    let kb_dir = meta_dir.join("kb");
    if kb_dir.exists() && kb_dir.is_dir() {
        Some(kb_dir)
    } else {
        None
    }
}

pub struct KbSummaries {
    pub active_summary: Option<String>,
    pub grave_summary: Option<String>,
}

pub fn get_kb_summaries() -> KbSummaries {
    let mut summaries = KbSummaries {
        active_summary: None,
        grave_summary: None,
    };

    if let Some(kb_dir) = get_kb_dir() {
        let knowledge_md = kb_dir.join("knowledge.md");
        if knowledge_md.exists() {
            if let Ok(content) = fs::read_to_string(&knowledge_md) {
                summaries.active_summary = Some(content);
            }
        }

        let grave_md = kb_dir.join("graveyard").join("grave.md");
        if grave_md.exists() {
            if let Ok(content) = fs::read_to_string(&grave_md) {
                summaries.grave_summary = Some(content);
            }
        }
    }

    summaries
}

/// Helper to sanitize and format date
fn current_date_str() -> String {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    // Just a simple timestamp for uniqueness or formatting
    since_the_epoch.as_secs().to_string()
}

fn sanitize_filename(title: &str) -> String {
    title.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .to_lowercase()
}

pub fn kb_search(query: &str, scope: Option<&str>, limit: usize) -> Result<String, String> {
    let kb_dir = match get_kb_dir() {
        Some(dir) => dir,
        None => return Ok("No active knowledge base found for this project.".to_string()),
    };

    let scope_str = scope.unwrap_or("all");
    let mut results = Vec::new();

    let mut search_dirs = Vec::new();
    if scope_str == "active" || scope_str == "all" {
        search_dirs.push(kb_dir.clone());
    }
    if scope_str == "grave" || scope_str == "all" {
        search_dirs.push(kb_dir.join("graveyard"));
    }

    let query_lower = query.to_lowercase();

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if content.to_lowercase().contains(&query_lower) {
                            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                            // Simple snippet extraction
                            let mut snippet = String::new();
                            for line in content.lines() {
                                if line.to_lowercase().contains(&query_lower) {
                                    snippet.push_str(line);
                                    snippet.push('\n');
                                    if snippet.len() > 200 {
                                        snippet.push_str("...\n");
                                        break;
                                    }
                                }
                            }

                            results.push(format!("--- {} ---\n{}", file_name, snippet));
                            if results.len() >= limit {
                                break;
                            }
                        }
                    }
                }
            }
        }
        if results.len() >= limit {
            break;
        }
    }

    if results.is_empty() {
        Ok("No matches found.".to_string())
    } else {
        Ok(results.join("\n"))
    }
}

pub fn kb_read(path: &str) -> Result<String, String> {
    let kb_dir = match get_kb_dir() {
        Some(dir) => dir,
        None => return Err("No active knowledge base found for this project.".to_string()),
    };

    let target_path = kb_dir.join(path);

    // Security check: ensure target_path is within kb_dir
    let normalized_target = target_path.canonicalize().unwrap_or_else(|_| target_path.clone());
    let normalized_kb = kb_dir.canonicalize().unwrap_or_else(|_| kb_dir.clone());

    if !normalized_target.starts_with(&normalized_kb) {
        return Err("Access denied: Path traversal detected.".to_string());
    }

    if !normalized_target.exists() || !normalized_target.is_file() {
        return Err(format!("File not found: {}", path));
    }

    fs::read_to_string(&normalized_target).map_err(|e| e.to_string())
}

pub fn kb_record(scope: &str, title: &str, summary: &str, content: &str) -> Result<String, String> {
    let kb_dir = match get_kb_dir() {
        Some(dir) => dir,
        None => return Err("No active knowledge base found for this project.".to_string()),
    };

    let (target_dir, index_file) = if scope == "grave" {
        (kb_dir.join("graveyard"), kb_dir.join("graveyard").join("grave.md"))
    } else {
        (kb_dir.clone(), kb_dir.join("knowledge.md"))
    };

    if !target_dir.exists() {
        fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    }

    let date_str = current_date_str();
    let filename = format!("{}_{}.md", sanitize_filename(title), date_str);
    let filepath = target_dir.join(&filename);

    let doc_content = format!(
        "---\ntitle: {}\nsummary: {}\ndate: {}\n---\n\n{}",
        title, summary, date_str, content
    );

    fs::write(&filepath, doc_content).map_err(|e| e.to_string())?;

    // Update index file
    let index_entry = format!("- [{}](./{}) - {}\n", title, filename, summary);
    if index_file.exists() {
        let mut index_content = fs::read_to_string(&index_file).map_err(|e| e.to_string())?;
        index_content.push_str(&index_entry);
        fs::write(&index_file, index_content).map_err(|e| e.to_string())?;
    } else {
        let index_content = format!("# Index\n\n{}", index_entry);
        fs::write(&index_file, index_content).map_err(|e| e.to_string())?;
    }

    Ok(format!("Successfully recorded to {}", filename))
}

pub fn kb_audit(proposal: &str) -> Result<String, String> {
    let kb_dir = match get_kb_dir() {
        Some(dir) => dir,
        None => return Ok("Audit skipped: No knowledge base found.".to_string()),
    };

    let grave_dir = kb_dir.join("graveyard");
    if !grave_dir.exists() {
        return Ok("No anti-patterns found in graveyard.".to_string());
    }

    let mut warnings = Vec::new();
    let proposal_lower = proposal.to_lowercase();

    if let Ok(entries) = fs::read_dir(&grave_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("md") {
                if let Ok(content) = fs::read_to_string(&path) {
                    // Extremely basic audit: check if grave title/summary keywords appear in proposal
                    // For better auditing, we'd use LLM or NLP, but we'll do simple keyword extraction here

                    // Extract frontmatter title and summary
                    let mut title = String::new();
                    for line in content.lines() {
                        if line.starts_with("title:") {
                            title = line.replace("title:", "").trim().to_lowercase();
                            break;
                        }
                    }

                    if !title.is_empty() && proposal_lower.contains(&title) {
                        warnings.push(format!("Anti-pattern warning: Proposal mentions '{}' which is in the graveyard ({})", title, path.file_name().unwrap_or_default().to_string_lossy()));
                    }
                }
            }
        }
    }

    if warnings.is_empty() {
        Ok("Audit passed: No anti-patterns detected.".to_string())
    } else {
        Ok(warnings.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_resolve_metadata_dir() {
        // Just verify it doesn't panic. Actual env resolution depends on environment.
        let dir = resolve_metadata_dir();
        assert!(dir.is_some());
    }

    #[test]
    fn test_kb_lifecycle() {
        let temp = tempdir().unwrap();
        let project_dir = temp.path().join(".gemini/Projects/test_project");
        let kb_dir = project_dir.join("kb");
        let grave_dir = kb_dir.join("graveyard");
        fs::create_dir_all(&grave_dir).unwrap();

        // Override env variables for test
        env::set_var("ENGINEDGE_META_DIR", project_dir.to_str().unwrap());

        let result = kb_record("active", "Arch", "Architecture rules", "No global state allowed");
        assert!(result.is_ok());

        let result = kb_record("grave", "GodObject", "Massive class", "Don't create a God object");
        assert!(result.is_ok());

        // Test search
        let search_res = kb_search("global", Some("all"), 5).unwrap();
        assert!(search_res.contains("global"));

        // Test read index files updated by record
        let active_md = fs::read_to_string(kb_dir.join("knowledge.md")).unwrap();
        assert!(active_md.contains("Arch"));

        let grave_md = fs::read_to_string(grave_dir.join("grave.md")).unwrap();
        assert!(grave_md.contains("GodObject"));

        // Test audit
        let audit_res = kb_audit("We need a GodObject to hold state").unwrap();
        assert!(audit_res.contains("warning"));

        // Test summaries
        let summaries = get_kb_summaries();
        assert!(summaries.active_summary.is_some());
        assert!(summaries.grave_summary.is_some());

        // Cleanup
        env::remove_var("ENGINEDGE_META_DIR");
    }
}

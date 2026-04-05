use anyhow::Result;
use glob::Pattern;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FindOptions {
    pub pattern: String,
    pub path: PathBuf,
    pub max_results: usize,
    pub ignore_patterns: Vec<String>,
}

pub fn find_files(options: &FindOptions) -> Result<Vec<PathBuf>> {
    let glob_pattern = Pattern::new(&options.pattern)?;

    let mut override_builder = OverrideBuilder::new(&options.path);
    for pat in &options.ignore_patterns {
        override_builder.add(&format!("!{}", pat))?;
    }
    let overrides = override_builder.build()?;

    let walker = WalkBuilder::new(&options.path)
        .git_ignore(true)
        .overrides(overrides)
        .build();

    let mut matches: Vec<(PathBuf, SystemTime)> = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
            continue;
        }

        let path = entry.path().to_path_buf();

        // Match against relative path from base and against just the filename
        let rel_path = path
            .strip_prefix(&options.path)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let matched = glob_pattern.matches(&rel_path) || glob_pattern.matches(&filename);

        if matched {
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            matches.push((path, mtime));
        }

        if matches.len() >= options.max_results * 10 {
            // Collect enough to sort, but stop early to avoid runaway
            break;
        }
    }

    // Sort by mtime descending (most recent first)
    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches.truncate(options.max_results);

    Ok(matches.into_iter().map(|(p, _)| p).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("lib.rs"), "pub fn foo() {}").unwrap();
        fs::write(dir.path().join("config.toml"), "[section]").unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        fs::write(sub.join("test.rs"), "mod tests {}").unwrap();
        dir
    }

    #[test]
    fn test_find_all_rs() {
        let dir = create_test_dir();
        let opts = FindOptions {
            pattern: "*.rs".to_string(),
            path: dir.path().to_path_buf(),
            max_results: 100,
            ignore_patterns: vec![],
        };
        let results = find_files(&opts).unwrap();
        assert_eq!(results.len(), 3, "Expected 3 .rs files, got: {:?}", results);
    }

    #[test]
    fn test_find_with_glob_path() {
        let dir = create_test_dir();
        let opts = FindOptions {
            pattern: "sub/*.rs".to_string(),
            path: dir.path().to_path_buf(),
            max_results: 100,
            ignore_patterns: vec![],
        };
        let results = find_files(&opts).unwrap();
        assert_eq!(results.len(), 1, "Expected 1 file matching sub/*.rs, got: {:?}", results);
        assert!(results[0].ends_with("test.rs"));
    }

    #[test]
    fn test_find_max_results() {
        let dir = create_test_dir();
        let opts = FindOptions {
            pattern: "*.rs".to_string(),
            path: dir.path().to_path_buf(),
            max_results: 2,
            ignore_patterns: vec![],
        };
        let results = find_files(&opts).unwrap();
        assert_eq!(results.len(), 2, "Expected exactly 2 results due to max_results");
    }

    #[test]
    fn test_find_no_matches() {
        let dir = create_test_dir();
        let opts = FindOptions {
            pattern: "*.xyz".to_string(),
            path: dir.path().to_path_buf(),
            max_results: 100,
            ignore_patterns: vec![],
        };
        let results = find_files(&opts).unwrap();
        assert!(results.is_empty(), "Expected no matches for *.xyz");
    }
}

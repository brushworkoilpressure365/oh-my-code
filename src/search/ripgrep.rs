use anyhow::Result;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GrepMatch {
    pub file_path: PathBuf,
    pub line_number: u64,
    pub line_content: String,
}

#[derive(Debug, Clone)]
pub struct GrepOptions {
    pub pattern: String,
    pub path: PathBuf,
    pub case_insensitive: bool,
    pub max_results: usize,
    pub glob_filter: Option<String>,
    pub ignore_patterns: Vec<String>,
}

pub fn grep_search(options: &GrepOptions) -> Result<Vec<GrepMatch>> {
    let pattern = if options.case_insensitive {
        format!("(?i){}", options.pattern)
    } else {
        options.pattern.clone()
    };

    let matcher = RegexMatcher::new(&pattern)?;

    let mut override_builder = OverrideBuilder::new(&options.path);
    for pat in &options.ignore_patterns {
        override_builder.add(&format!("!{}", pat))?;
    }
    if let Some(ref glob) = options.glob_filter {
        override_builder.add(glob)?;
    }
    let overrides = override_builder.build()?;

    let walker = WalkBuilder::new(&options.path)
        .git_ignore(true)
        .overrides(overrides)
        .build();

    let mut results: Vec<GrepMatch> = Vec::new();
    let max = options.max_results;

    'outer: for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().map(|t| !t.is_file()).unwrap_or(true) {
            continue;
        }

        let path = entry.path().to_path_buf();
        let mut searcher = Searcher::new();

        searcher.search_path(
            &matcher,
            &path,
            UTF8(|line_num, line_content| {
                if results.len() >= max {
                    return Ok(false);
                }
                results.push(GrepMatch {
                    file_path: path.clone(),
                    line_number: line_num,
                    line_content: line_content.trim_end_matches('\n').to_string(),
                });
                Ok(true)
            }),
        ).ok();

        if results.len() >= max {
            break 'outer;
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_dir() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file1.txt"), "Hello World\nfoo bar\n").unwrap();
        fs::write(dir.path().join("file2.txt"), "hello again\nbaz qux\n").unwrap();
        dir
    }

    #[test]
    fn test_grep_basic() {
        let dir = create_test_dir();
        let opts = GrepOptions {
            pattern: "hello".to_string(),
            path: dir.path().to_path_buf(),
            case_insensitive: true,
            max_results: 100,
            glob_filter: None,
            ignore_patterns: vec![],
        };
        let results = grep_search(&opts).unwrap();
        // Both "Hello World" and "hello again" should match case-insensitively
        assert!(results.len() >= 2, "Expected at least 2 matches, got {}", results.len());
    }

    #[test]
    fn test_grep_case_sensitive() {
        let dir = create_test_dir();
        let opts = GrepOptions {
            pattern: "Hello".to_string(),
            path: dir.path().to_path_buf(),
            case_insensitive: false,
            max_results: 100,
            glob_filter: None,
            ignore_patterns: vec![],
        };
        let results = grep_search(&opts).unwrap();
        // Only "Hello World" should match exactly
        assert_eq!(results.len(), 1, "Expected exactly 1 match, got {}", results.len());
        assert!(results[0].line_content.contains("Hello"));
    }

    #[test]
    fn test_grep_max_results() {
        let dir = create_test_dir();
        let opts = GrepOptions {
            pattern: "hello".to_string(),
            path: dir.path().to_path_buf(),
            case_insensitive: true,
            max_results: 1,
            glob_filter: None,
            ignore_patterns: vec![],
        };
        let results = grep_search(&opts).unwrap();
        assert_eq!(results.len(), 1, "Expected exactly 1 result due to max_results limit");
    }

    #[test]
    fn test_grep_no_matches() {
        let dir = create_test_dir();
        let opts = GrepOptions {
            pattern: "zzznomatchzzz".to_string(),
            path: dir.path().to_path_buf(),
            case_insensitive: false,
            max_results: 100,
            glob_filter: None,
            ignore_patterns: vec![],
        };
        let results = grep_search(&opts).unwrap();
        assert!(results.is_empty(), "Expected no matches");
    }
}

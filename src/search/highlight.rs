use anyhow::Result;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

pub struct Highlighter {
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
}

impl Highlighter {
    pub fn new() -> Result<Self> {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        Ok(Self {
            syntax_set,
            theme_set,
        })
    }

    pub fn highlight_file(&self, path: &Path, content: &str) -> String {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("txt");

        let syntax = self
            .syntax_set
            .find_syntax_by_extension(extension)
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        let theme = &self.theme_set.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut output = String::new();
        for (line_num, line) in LinesWithEndings::from(content).enumerate() {
            let ranges: Vec<(Style, &str)> = highlighter
                .highlight_line(line, &self.syntax_set)
                .unwrap_or_default();
            let escaped = as_24_bit_terminal_escaped(&ranges[..], false);
            output.push_str(&format!("{:4} | {}", line_num + 1, escaped));
        }
        output
    }

    pub fn with_line_numbers(content: &str) -> String {
        let mut output = String::new();
        for (line_num, line) in content.lines().enumerate() {
            output.push_str(&format!("{:4} | {}\n", line_num + 1, line));
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_highlighter_new() {
        let h = Highlighter::new().expect("Should create Highlighter");
        assert!(!h.syntax_set.syntaxes().is_empty(), "Syntax set should not be empty");
    }

    #[test]
    fn test_highlight_file_rust() {
        let h = Highlighter::new().expect("Should create Highlighter");
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let path = Path::new("test.rs");
        let result = h.highlight_file(path, content);
        // Should contain line numbers
        assert!(result.contains("   1 |"), "Should contain line number 1, got: {}", result);
        assert!(result.contains("   2 |"), "Should contain line number 2, got: {}", result);
    }

    #[test]
    fn test_with_line_numbers() {
        let content = "line one\nline two\n";
        let result = Highlighter::with_line_numbers(content);
        assert!(result.contains("   1 | line one"), "Should contain '   1 | line one', got: {}", result);
        assert!(result.contains("   2 | line two"), "Should contain '   2 | line two', got: {}", result);
    }
}

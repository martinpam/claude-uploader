use glob::Pattern;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ClaudeKeepConfig {
    pub sections: Vec<String>,
    pub patterns: HashMap<String, Vec<String>>,
}

impl ClaudeKeepConfig {
    pub fn from_file(folder_path: &Path) -> Option<Self> {
        let keep_path = folder_path.join(".claudekeep");
        if !keep_path.exists() {
            return None;
        }

        let content = fs::read_to_string(keep_path).ok()?;
        let mut config = ClaudeKeepConfig::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line.ends_with(':') {
                current_section = line[..line.len() - 1].to_string();
                config.sections.push(current_section.clone());
                config.patterns.insert(current_section.clone(), Vec::new());
            } else if !current_section.is_empty() {
                if let Some(patterns) = config.patterns.get_mut(&current_section) {
                    patterns.push(line.to_string());
                }
            }
        }

        Some(config)
    }

    pub fn should_include_file(&self, file_path: &Path, selected_sections: &[String]) -> bool {
        if selected_sections.is_empty() {
            return true; // If no sections selected, include all files
        }

        let relative_path = if let Ok(canonical_path) = file_path.canonicalize() {
            canonical_path
        } else {
            return false;
        };

        for section in selected_sections {
            if let Some(patterns) = self.patterns.get(section) {
                for pattern in patterns {
                    if Self::matches_pattern(&relative_path, pattern) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn matches_pattern(file_path: &Path, pattern: &str) -> bool {
        let pattern = if pattern.contains('*') {
            Pattern::new(pattern)
        } else {
            Pattern::new(&format!("**/{}", pattern))
        };

        if let Ok(pattern) = pattern {
            pattern.matches_path(file_path)
        } else {
            false
        }
    }
}

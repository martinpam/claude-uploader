use glob::Pattern;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone)]
pub struct ClaudeKeepConfig {
    pub sections: Vec<String>,
    pub patterns: HashMap<String, Vec<String>>,
    folder_path: PathBuf,
}

impl ClaudeKeepConfig {
    pub fn from_file(folder_path: &Path) -> Option<Self> {
        let keep_path = folder_path.join(".claudekeep");
        println!("Reading .claudekeep from: {:?}", keep_path);

        if !keep_path.exists() {
            return None;
        }

        let content = fs::read_to_string(keep_path).ok()?;
        println!("File content:\n{}", content);

        let mut config = ClaudeKeepConfig {
            sections: Vec::new(),
            patterns: HashMap::new(),
            folder_path: folder_path.to_path_buf(),
        };

        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            println!("Processing line: {}", line);
            if line.ends_with(':') {
                current_section = line[..line.len() - 1].to_string();
                config.sections.push(current_section.clone());
                config.patterns.insert(current_section.clone(), Vec::new());
                // println!("New section: {}", current_section);
            } else if !current_section.is_empty() {
                if let Some(patterns) = config.patterns.get_mut(&current_section) {
                    patterns.push(line.to_string());
                    // println!("Added pattern: {} to section: {}", line, current_section);
                }
            }
        }

        println!("Final config: {:?}", config);
        Some(config)
    }

    pub fn should_include_file(&self, file_path: &Path, selected_sections: &[String]) -> bool {
        // println!("Checking file: {:?}", file_path);
        // println!("Selected sections: {:?}", selected_sections);

        if selected_sections.is_empty() {
            // println!("No sections selected, including file");
            return true;
        }

        let relative_path = if let Ok(canonical_path) = file_path.canonicalize() {
            if let Ok(relative) = canonical_path.strip_prefix(&self.folder_path) {
                // println!("Relative path: {:?}", relative);
                relative.to_path_buf()
            } else {
                // println!("Failed to create relative path");
                return false;
            }
        } else {
            // println!("Failed to canonicalize path");
            return false;
        };

        for section in selected_sections {
            // println!("Checking section: {}", section);
            if let Some(patterns) = self.patterns.get(section) {
                for pattern in patterns {
                    // println!("Trying pattern: {}", pattern);
                    let processed_pattern = if pattern.starts_with("**/") {
                        pattern.to_string()
                    } else {
                        format!("**/{}", pattern)
                    };
                    // println!("Processed pattern: {}", processed_pattern);

                    if let Ok(glob_pattern) = Pattern::new(&processed_pattern) {
                        if glob_pattern.matches_path(&relative_path) {
                            // println!("✅ Matched!");
                            return true;
                        }
                        // println!("❌ No match");
                    } else {
                        // println!("Invalid pattern: {}", pattern);
                    }
                }
            }
        }

        // println!("No patterns matched for file");
        false
    }
}

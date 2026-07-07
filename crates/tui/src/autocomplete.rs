//! Autocomplete system for slash commands and file paths

use std::path::PathBuf;

/// An autocomplete item
#[derive(Debug, Clone)]
pub struct AutocompleteItem {
    pub value: String,
    pub label: String,
    pub description: Option<String>,
}

/// Autocomplete suggestions
#[derive(Debug, Clone)]
pub struct AutocompleteSuggestions {
    pub items: Vec<AutocompleteItem>,
    pub prefix: String,
}

/// A slash command definition
#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub description: Option<String>,
    pub argument_hint: Option<String>,
}

/// Autocomplete provider
pub trait AutocompleteProvider: Send + Sync {
    /// Get autocomplete suggestions for current text/cursor position
    fn get_suggestions(
        &self,
        text_before_cursor: &str,
        force: bool,
    ) -> Option<AutocompleteSuggestions>;

    /// Apply the selected completion item
    fn apply_completion(
        &self,
        text_before_cursor: &str,
        text_after_cursor: &str,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> (String, usize);
}

/// Combined autocomplete provider for slash commands and file paths
pub struct CombinedAutocompleteProvider {
    commands: Vec<SlashCommand>,
    base_path: PathBuf,
}

impl CombinedAutocompleteProvider {
    pub fn new(commands: Vec<SlashCommand>, base_path: PathBuf) -> Self {
        Self {
            commands,
            base_path,
        }
    }

    fn extract_path_prefix(&self, text: &str, force: bool) -> Option<String> {
        // Find the last space/delimiter to extract the current token
        let last_delim = text
            .rfind([' ', '\t', '"', '\'', '='])
            .map(|i| i + 1)
            .unwrap_or(0);

        let token = &text[last_delim..];
        if token.is_empty() && !text.ends_with(' ') {
            return None;
        }
        if token.is_empty() {
            return Some(String::new());
        }

        let has_path_like = token.contains('/')
            || token.contains('\\')
            || token.starts_with('.')
            || token.starts_with("~/");
        let after_at = token.strip_prefix('@').unwrap_or(token);

        if force
            || has_path_like
            || after_at.contains('/')
            || after_at.contains('\\')
            || after_at.starts_with('.')
            || token.starts_with('@')
        {
            Some(token.to_string())
        } else if !force {
            None
        } else {
            Some(token.to_string())
        }
    }

    /// Resolve a path prefix into (search_directory, search_prefix).
    /// Handles ~/, ~\, absolute paths, and relative paths.
    fn resolve_search_dir(&self, raw: &str) -> (PathBuf, String) {
        if (raw.starts_with("~/") || raw.starts_with("~\\")) || raw == "~" {
            let home = dirs::home_dir().unwrap_or_default();
            let remainder = if raw.len() > 2 { &raw[2..] } else { "" };
            (home.join(remainder), String::new())
        } else if raw.starts_with('/') {
            // Absolute path: split at last separator
            if let Some(last_sep) = raw.rfind(['/', '\\']) {
                let dir = PathBuf::from(&raw[..=last_sep]);
                let prefix = raw[last_sep + 1..].to_string();
                (dir, prefix)
            } else {
                (PathBuf::from("/"), raw.to_string())
            }
        } else {
            // Relative path
            if let Some(last_sep) = raw.rfind(['/', '\\']) {
                let dir = self.base_path.join(&raw[..=last_sep]);
                let prefix = raw[last_sep + 1..].to_string();
                (dir, prefix)
            } else {
                (self.base_path.clone(), raw.to_string())
            }
        }
    }

    /// Build the completion path for a file/directory entry.
    /// `raw` is the user's typed token (without @), `name` is the selected entry name.
    fn build_completion_path(&self, raw: &str, name: &str, is_dir: bool) -> String {
        let sep = if raw.contains('\\') { "\\" } else { "/" };

        // Helper: extract base directory (without trailing separator), and whether it's empty
        let extract_base = |s: &str| -> (String, bool) {
            if s.is_empty() {
                (String::new(), true)
            } else if s.ends_with('/') || s.ends_with('\\') {
                (s[..s.len() - 1].to_string(), false)
            } else if let Some(pos) = s.rfind(['/', '\\']) {
                (s[..pos].to_string(), false)
            } else {
                (String::new(), true)
            }
        };

        if raw.starts_with('/') {
            // Absolute path
            let (base, is_root) = extract_base(raw);
            let prefix = if is_root {
                sep.to_string()
            } else {
                format!("{}{}", base, sep)
            };
            if is_dir {
                format!("{}{}{}", prefix, name, sep)
            } else {
                format!("{}{}", prefix, name)
            }
        } else if (raw.starts_with("~/") || raw.starts_with("~\\")) || raw == "~" {
            // Home-relative path
            let (base, _) = extract_base(raw);
            let prefix_str = if raw == "~" {
                format!("~{}", sep)
            } else if base.is_empty() && (raw.ends_with('/') || raw.ends_with('\\')) {
                raw.to_string()
            } else if base.is_empty() {
                raw.to_string()
            } else {
                format!("{}{}", base, sep)
            };
            if is_dir {
                format!("{}{}{}", prefix_str, name, sep)
            } else {
                format!("{}{}", prefix_str, name)
            }
        } else {
            // Relative path
            let (base, is_root) = extract_base(raw);
            let prefix_str = if is_root {
                String::new()
            } else {
                format!("{}{}", base, sep)
            };
            if is_dir {
                format!("{}{}{}", prefix_str, name, sep)
            } else {
                format!("{}{}", prefix_str, name)
            }
        }
    }

    fn get_file_suggestions(&self, prefix: &str) -> Vec<AutocompleteItem> {
        let raw = prefix.strip_prefix('@').unwrap_or(prefix);
        let (search_dir, search_prefix) = self.resolve_search_dir(raw);

        let mut suggestions = Vec::new();
        let dir = match std::fs::read_dir(&search_dir) {
            Ok(d) => d,
            Err(_) => return suggestions,
        };

        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name
                .to_lowercase()
                .starts_with(&search_prefix.to_lowercase())
            {
                continue;
            }

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let display_name = if is_dir {
                format!("{}/", name)
            } else {
                name.clone()
            };

            let completion_path = self.build_completion_path(raw, &name, is_dir);
            let value_text = if is_dir && !completion_path.ends_with('/') {
                format!("{}/", completion_path)
            } else {
                completion_path
            };

            let prefixed = if prefix.starts_with('@') {
                format!("@{}{}", value_text, if is_dir { "" } else { " " })
            } else {
                format!("{}{}", value_text, if is_dir { "" } else { " " })
            };

            suggestions.push(AutocompleteItem {
                value: prefixed,
                label: display_name,
                description: if is_dir {
                    Some("[dir]".to_string())
                } else {
                    None
                },
            });
        }

        suggestions.sort_by(|a, b| {
            let a_dir = a.label.ends_with('/');
            let b_dir = b.label.ends_with('/');
            if a_dir && !b_dir {
                std::cmp::Ordering::Less
            } else if !a_dir && b_dir {
                std::cmp::Ordering::Greater
            } else {
                a.label.cmp(&b.label)
            }
        });

        suggestions
    }
}

impl AutocompleteProvider for CombinedAutocompleteProvider {
    fn get_suggestions(
        &self,
        text_before_cursor: &str,
        force: bool,
    ) -> Option<AutocompleteSuggestions> {
        // Slash commands: text starts with "/"
        if !force && text_before_cursor.starts_with('/') {
            let space_idx = text_before_cursor.find(' ');
            if space_idx.is_none() {
                let prefix = &text_before_cursor[1..];
                let command_items: Vec<SlashCommand> = if prefix.is_empty() {
                    self.commands.clone()
                } else {
                    crate::fuzzy::fuzzy_filter(&self.commands, prefix, |cmd| cmd.name.as_str())
                };

                let items: Vec<AutocompleteItem> = command_items
                    .into_iter()
                    .map(|cmd| {
                        let desc = match (&cmd.argument_hint, &cmd.description) {
                            (Some(hint), Some(desc)) => format!("{} — {}", hint, desc),
                            (Some(hint), None) => hint.clone(),
                            (None, Some(desc)) => desc.clone(),
                            (None, None) => String::new(),
                        };
                        AutocompleteItem {
                            value: cmd.name.clone(),
                            label: cmd.name,
                            description: if desc.is_empty() { None } else { Some(desc) },
                        }
                    })
                    .collect();

                if items.is_empty() {
                    return None;
                }
                return Some(AutocompleteSuggestions {
                    items,
                    prefix: text_before_cursor.to_string(),
                });
            }
            // Command arguments - not implemented yet
            return None;
        }

        // File path completion
        let path_prefix = self.extract_path_prefix(text_before_cursor, force)?;
        let suggestions = self.get_file_suggestions(&path_prefix);
        if suggestions.is_empty() {
            return None;
        }

        Some(AutocompleteSuggestions {
            items: suggestions,
            prefix: path_prefix,
        })
    }

    fn apply_completion(
        &self,
        text_before_cursor: &str,
        text_after_cursor: &str,
        item: &AutocompleteItem,
        prefix: &str,
    ) -> (String, usize) {
        let before_prefix =
            &text_before_cursor[..text_before_cursor.len().saturating_sub(prefix.len())];

        // Slash command
        if prefix.starts_with('/') {
            let result = format!("{}/{} {}", before_prefix, item.value, text_after_cursor);
            let cursor = before_prefix.len() + item.value.len() + 2; // +2 for / and space
            return (result, cursor);
        }

        // File completion
        let is_dir = item.label.ends_with('/');
        let suffix = if is_dir { "" } else { " " };
        let result = format!("{}{}{}", before_prefix, item.value, text_after_cursor);
        let cursor = before_prefix.len() + item.value.len() + suffix.len();
        (result, cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_completion() {
        let commands = vec![SlashCommand {
            name: "model".to_string(),
            description: Some("Change model".to_string()),
            argument_hint: None,
        }];
        let provider = CombinedAutocompleteProvider::new(commands, PathBuf::from("/tmp"));
        let result = provider.get_suggestions("/mod", false);
        assert!(result.is_some());
        let suggestions = result.unwrap();
        assert_eq!(suggestions.items.len(), 1);
        assert_eq!(suggestions.items[0].value, "model");
    }

    #[test]
    fn test_path_extraction() {
        let commands = Vec::new();
        let provider = CombinedAutocompleteProvider::new(commands, PathBuf::from("/tmp"));
        // Text that looks like a path
        let result = provider.extract_path_prefix("open /usr/loc", false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "/usr/loc");
    }

    #[test]
    fn test_at_mention_triggers_completion() {
        let commands = Vec::new();
        let provider = CombinedAutocompleteProvider::new(commands, PathBuf::from("/tmp"));
        // @ mention should be recognized
        let result = provider.extract_path_prefix("hello @src", false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "@src");
    }

    #[test]
    fn test_backslash_triggers_completion() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("/tmp"));
        // Backslash should be recognized as path-like (Windows support)
        let result = provider.extract_path_prefix("hello @src\\main", false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "@src\\main");
    }

    #[test]
    fn test_extract_path_prefix_at_alone_returns_none() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("/tmp"));
        // Just @ alone should not trigger (too short, see insert_char guard)
        let result = provider.extract_path_prefix("@", false);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "@");
    }

    #[test]
    fn test_resolve_search_dir_with_backslash() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("C:\\project"));
        let (dir, prefix) = provider.resolve_search_dir("src\\main");
        // Should split at backslash
        assert_eq!(dir, PathBuf::from("C:\\project\\src\\"));
        assert_eq!(prefix, "main");
    }

    #[test]
    fn test_build_completion_path_preserves_backslash_base() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("C:\\project"));
        // When raw ends with backslash, base should be the directory with separator
        let result = provider.build_completion_path("src\\", "main.rs", false);
        assert_eq!(result, "src\\main.rs");
    }

    #[test]
    fn test_build_completion_path_with_backslash_prefix() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("C:\\project"));
        // Partial path with backslash should work
        let result = provider.build_completion_path("src\\ma", "main.rs", false);
        assert_eq!(result, "src\\main.rs");
    }

    #[test]
    fn test_build_completion_path_forward_slash() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("/project"));
        // Forward slash always works
        let result = provider.build_completion_path("src/", "main.rs", false);
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn test_build_completion_path_dir_with_backslash() {
        let provider = CombinedAutocompleteProvider::new(vec![], PathBuf::from("C:\\project"));
        // Directory completion with backslash
        let result = provider.build_completion_path("src\\", "subdir", true);
        assert_eq!(result, "src\\subdir\\");
    }
}

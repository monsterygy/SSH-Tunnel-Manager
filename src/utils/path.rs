use std::path::{Path, PathBuf};

/// Expand tilde (~) in file paths to the user's home directory.
pub fn expand_tilde(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if let Some(rest) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if path_str == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde(Path::new("~/.ssh/id_rsa"));
        assert!(!expanded.to_string_lossy().starts_with('~'));
        assert!(expanded.to_string_lossy().ends_with(".ssh/id_rsa"));
    }

    #[test]
    fn test_no_tilde() {
        let path = Path::new("/absolute/path");
        assert_eq!(expand_tilde(path), PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_tilde_only() {
        let expanded = expand_tilde(Path::new("~"));
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }
}

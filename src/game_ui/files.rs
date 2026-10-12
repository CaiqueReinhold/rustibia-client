use std::io;
use std::path::Path;

/// Writes through a temporary file renamed over `path`, so a write cut short never leaves a
/// truncated file behind.
pub(crate) fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staged = path.with_extension("tmp");
    std::fs::write(&staged, contents)?;
    std::fs::rename(&staged, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustibia-files-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_write_creates_missing_directories() {
        let path = scratch_dir("creates").join("nested").join("file.json");

        write_atomically(&path, "first").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
    }

    #[test]
    fn a_write_replaces_the_file_and_leaves_no_staging_file() {
        let path = scratch_dir("replaces").join("file.json");
        write_atomically(&path, "first").unwrap();

        write_atomically(&path, "second").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
        assert!(!path.with_extension("tmp").exists());
    }
}

use std::path::PathBuf;

/// Search PATH for an executable by name. Returns the full path if found.
pub fn find_binary(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .map(|dir| PathBuf::from(dir).join(name))
        .find(|path| path.is_file())
}

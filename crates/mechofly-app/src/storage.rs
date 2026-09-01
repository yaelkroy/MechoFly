use std::path::PathBuf;

pub fn override_directory() -> Option<PathBuf> {
    std::env::var_os("MECHOFLY_DATA_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

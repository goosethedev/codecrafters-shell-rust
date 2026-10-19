use std::path::Path;

pub fn is_executable(path: impl AsRef<Path>) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match path.as_ref().metadata() {
        Ok(meta) if meta.is_file() => meta.permissions().mode() & 0o111 != 0,
        _ => false,
    }
}

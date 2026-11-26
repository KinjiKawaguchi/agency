//! Writing a source's usage file.
use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// Writes `usage` to `path` in one step, so a reader sees the old file or the new one, never half of
/// it. Only the node's account reads the file.
pub fn write(path: &Path, usage: &Value) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let temporary = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("usage"),
        std::process::id()
    ));
    let _ = std::fs::remove_file(&temporary);
    let result = options
        .open(&temporary)
        .and_then(|mut file| file.write_all(usage.to_string().as_bytes()))
        .and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replaces_the_file_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("usage").join("a.json");
        write(&path, &json!({"five_hour": {"utilization": 1}})).unwrap();
        write(&path, &json!({"five_hour": {"utilization": 2}})).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"five_hour":{"utilization":2}}"#
        );
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
        #[cfg(unix)]
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(
                &std::fs::metadata(&path).unwrap().permissions()
            ) & 0o777,
            0o600
        );
    }
}

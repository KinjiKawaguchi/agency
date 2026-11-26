//! How this node was started: the running program and its data directory. A client shows the owner the
//! exact command to approve a device at the node's terminal, which only the node can tell.
use std::path::Path;

/// The absolute path of the running program and of the data directory it uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Launch {
    pub executable: String,
    pub data_dir: String,
}

impl Launch {
    /// This process: `current_exe`, and `data_dir` made absolute. `None` when either is not a path that
    /// can be written out as text, so a client never shows a path that is not the real one.
    pub fn of_process(data_dir: &Path) -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        let data_dir = std::path::absolute(data_dir).ok()?;
        Some(Self {
            executable: executable.to_str()?.to_owned(),
            data_dir: data_dir.to_str()?.to_owned(),
        })
    }

    /// What to run to approve a device: the program, `--dir` with the data directory unless it is
    /// `<home>/.agency` (the one a terminal uses without `--dir` or `AGENCY_DIR`; a terminal may not
    /// have the node's environment, so `AGENCY_DIR` does not count), and `device-add`.
    pub fn device_add_command(&self, home: Option<&Path>) -> String {
        let is_default =
            home.is_some_and(|home| Path::new(&self.data_dir) == home.join(".agency").as_path());
        let mut words = vec![shell_word(&self.executable)];
        if !is_default {
            words.push("--dir".to_owned());
            words.push(shell_word(&self.data_dir));
        }
        words.push("device-add".to_owned());
        words.join(" ")
    }
}

/// A word for a POSIX shell: as it is when it holds only characters the shell leaves alone, otherwise
/// in single quotes (a single quote inside becomes `'\''`).
fn shell_word(text: &str) -> String {
    let is_plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c));
    if is_plain {
        text.to_owned()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch(executable: &str, data_dir: &str) -> Launch {
        Launch {
            executable: executable.into(),
            data_dir: data_dir.into(),
        }
    }

    #[test]
    fn the_default_data_directory_is_left_out() {
        let at = launch("/usr/local/bin/agency", "/home/alice/.agency");
        assert_eq!(
            at.device_add_command(Some(Path::new("/home/alice"))),
            "/usr/local/bin/agency device-add"
        );
    }

    #[test]
    fn another_data_directory_is_named() {
        let at = launch("/usr/local/bin/agency", "/var/agency");
        assert_eq!(
            at.device_add_command(Some(Path::new("/home/alice"))),
            "/usr/local/bin/agency --dir /var/agency device-add"
        );
        let other_home = launch("/usr/local/bin/agency", "/home/bob/.agency");
        assert_eq!(
            other_home.device_add_command(Some(Path::new("/home/alice"))),
            "/usr/local/bin/agency --dir /home/bob/.agency device-add",
            "the default is the home of the one who runs it, not any .agency"
        );
        assert!(at.device_add_command(None).contains("--dir /var/agency"));
    }

    #[test]
    fn a_path_with_a_space_or_a_quote_is_quoted() {
        let at = launch("/Applications/My Agency/agency", "/Users/a b/it's");
        assert_eq!(
            at.device_add_command(None),
            r"'/Applications/My Agency/agency' --dir '/Users/a b/it'\''s' device-add"
        );
    }

    #[test]
    fn this_process_is_described_by_absolute_paths() {
        let at = Launch::of_process(Path::new("relative/dir")).unwrap();
        assert!(Path::new(&at.executable).is_absolute());
        assert!(Path::new(&at.data_dir).is_absolute() && at.data_dir.ends_with("relative/dir"));
    }
}

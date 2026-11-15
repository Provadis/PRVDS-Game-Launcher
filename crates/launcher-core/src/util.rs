use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as Unix seconds.
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Normalises a game name for matching: lower case, trademark signs and
/// punctuation removed, whitespace collapsed.
pub fn normalize_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut last_space = true;
    for c in name.chars() {
        let c = match c {
            '™' | '®' | '©' => continue,
            c if c.is_alphanumeric() => {
                last_space = false;
                out.extend(c.to_lowercase());
                continue;
            }
            _ => ' ',
        };
        if !last_space {
            out.push(c);
            last_space = true;
        }
    }
    out.trim_end().to_string()
}

/// Turns a shortcut file name into a readable game title.
pub fn title_from_file_stem(stem: &str) -> String {
    let mut s = stem.trim().to_string();
    for suffix in [
        " - Shortcut",
        " - Verknüpfung",
        " – Verknüpfung",
        " Shortcut",
        " Verknüpfung",
    ] {
        if let Some(stripped) = s.strip_suffix(suffix) {
            s = stripped.trim().to_string();
        }
    }
    s
}

/// Case-insensitive "path starts with" for Windows style paths stored as strings.
pub fn path_starts_with(path: &str, prefix: &str) -> bool {
    let norm = |p: &str| p.replace('/', "\\").trim_end_matches('\\').to_lowercase();
    let path = norm(path);
    let prefix = norm(prefix);
    !prefix.is_empty() && (path == prefix || path.starts_with(&(prefix + "\\")))
}

/// Last path component of a Windows or Unix path string.
pub fn file_name_of(path: &str) -> &str {
    path.rsplit(['\\', '/']).next().unwrap_or(path)
}

/// Parent directory of a Windows or Unix path string.
pub fn parent_of(path: &str) -> Option<&str> {
    let idx = path.rfind(['\\', '/'])?;
    Some(&path[..idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_names() {
        assert_eq!(normalize_name("Baldur's Gate 3"), "baldur s gate 3");
        assert_eq!(normalize_name("ELDEN RING™"), "elden ring");
        assert_eq!(normalize_name("  Diablo®  IV "), "diablo iv");
    }

    #[test]
    fn strips_shortcut_suffix() {
        assert_eq!(title_from_file_stem("Valheim - Shortcut"), "Valheim");
        assert_eq!(title_from_file_stem("Diablo IV - Verknüpfung"), "Diablo IV");
        assert_eq!(title_from_file_stem("Hades II"), "Hades II");
    }

    #[test]
    fn path_prefix() {
        assert!(path_starts_with(
            r"C:\Games\Foo\bin\foo.exe",
            r"c:\games\foo"
        ));
        assert!(path_starts_with(r"C:\Games\Foo", r"C:\Games\Foo\"));
        assert!(!path_starts_with(r"C:\Games\Foobar\x.exe", r"C:\Games\Foo"));
        assert!(!path_starts_with(r"C:\x", ""));
    }

    #[test]
    fn path_parts() {
        assert_eq!(file_name_of(r"C:\a\b.exe"), "b.exe");
        assert_eq!(parent_of(r"C:\a\b.exe"), Some(r"C:\a"));
    }
}

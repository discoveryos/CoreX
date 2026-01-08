/// Generic file path/name sanitization
/// (C) 2025  kevin danthew

/// The root path fallback
pub const ROOT: &str = "/";

/// Strip the mountpoint prefix from a filename.
/// Returns a slice of the original string.
pub fn fs_strip_mountpoint<'a>(filename: &'a str, mnt: &MountPoint) -> &'a str {
    let prefix_len = mnt.prefix.len() - 1; // remove trailing slash
    if filename.len() > prefix_len {
        &filename[prefix_len..]
    } else {
        ROOT
    }
}

/// Copy filename into safe buffer while removing redundant slashes and `./`
pub fn fs_sanitize_copy_safe(filename: &str) -> String {
    let mut safe = String::with_capacity(filename.len());
    let bytes = filename.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'/' {
            // skip double slashes
            if i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                i += 1;
                continue;
            }
            // skip trailing slash
            if i + 1 == bytes.len() {
                break;
            }
            // skip `/.` patterns
            if i + 2 <= bytes.len() && bytes[i + 1] == b'.' && (bytes[i + 2] == b'/' || i + 2 == bytes.len()) {
                i += 2;
                continue;
            }
        }
        safe.push(bytes[i] as char);
        i += 1;
    }

    if safe.is_empty() {
        safe.push('/');
    }

    safe
}

/// Sanitize a filename relative to a prefix (e.g., current working directory)
pub fn fs_sanitize(prefix: &str, filename: &str) -> String {
    let mut safe_filename = if !filename.starts_with('/') {
        // Relative path: prepend prefix
        let mut combined = String::with_capacity(prefix.len() + 1 + filename.len());
        combined.push_str(prefix);
        if !prefix.ends_with('/') {
            combined.push('/');
        }
        combined.push_str(&fs_sanitize_copy_safe(filename));
        fs_sanitize_copy_safe(&combined)
    } else {
        // Absolute path
        fs_sanitize_copy_safe(filename)
    };

    // Resolve any `..` patterns
    loop {
        if let Some(pos) = safe_filename.find("/../") {
            // find previous slash before pos
            let prev_slash = safe_filename[..pos].rfind('/').unwrap_or(0);
            safe_filename.replace_range(prev_slash..pos + 3, "");
        } else if safe_filename.ends_with("/..") {
            let prev_slash = safe_filename[..safe_filename.len() - 3]
                .rfind('/')
                .unwrap_or(0);
            safe_filename.replace_range(prev_slash.., "");
        } else {
            break;
        }
    }

    if safe_filename.is_empty() {
        safe_filename.push('/');
    }

    safe_filename
}

/// MountPoint stub for demonstration
pub struct MountPoint {
    pub prefix: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize() {
        let cwd = "/home/user";
        assert_eq!(fs_sanitize(cwd, "file.txt"), "/home/user/file.txt");
        assert_eq!(fs_sanitize(cwd, "./file.txt"), "/home/user/file.txt");
        assert_eq!(fs_sanitize(cwd, "/etc/config"), "/etc/config");
        assert_eq!(fs_sanitize(cwd, "a//b///c/"), "/home/user/a/b/c");
        assert_eq!(fs_sanitize(cwd, "a/../b"), "/home/user/b");
    }
}

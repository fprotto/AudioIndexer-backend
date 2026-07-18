use std::path::{Component, Path, PathBuf};

/// Errors that can occur while resolving a user-supplied relative path
/// against the server's music root.
#[derive(Debug)]
pub enum PathError {
    /// The path escapes the root directory (e.g. via `..` or an absolute path).
    Traversal,
    /// The resolved path does not exist on disk.
    NotFound,
    /// The path exists but couldn't be canonicalized (permissions, symlink loop, etc).
    Io(std::io::Error),
}

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathError::Traversal => write!(f, "path escapes the music root"),
            PathError::NotFound => write!(f, "path not found"),
            PathError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

/// Resolves a user-supplied relative path (e.g. taken from a URL tail like
/// `foo/bar/baz.mp3`) against `root`, guaranteeing the result stays inside
/// `root`.
///
/// Two layers of defense are used:
/// 1. Lexical rejection of `..`, `.`, prefix/root components, before touching
///    the filesystem at all.
/// 2. Canonicalization of both the root and the resolved path, followed by a
///    check that the resolved path is actually a descendant of the
///    canonical root. This also defeats symlinks that point outside the
///    root.
///
/// The path does not need to exist for step 1; step 2 requires it to exist
/// because `canonicalize` fails otherwise.
pub fn resolve_within_root(root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let mut safe_rel = PathBuf::new();

    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => safe_rel.push(part),
            // Reject ".." (would escape), "." is a no-op we can skip,
            // and reject any absolute-path components ("/", "C:\", etc).
            Component::ParentDir => return Err(PathError::Traversal),
            Component::CurDir => continue,
            Component::RootDir | Component::Prefix(_) => return Err(PathError::Traversal),
        }
    }

    let candidate = root.join(&safe_rel);

    let canonical_root = root.canonicalize().map_err(PathError::Io)?;
    let canonical_candidate = match candidate.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(PathError::NotFound),
        Err(e) => return Err(PathError::Io(e)),
    };

    if canonical_candidate.starts_with(&canonical_root) {
        Ok(canonical_candidate)
    } else {
        Err(PathError::Traversal)
    }
}

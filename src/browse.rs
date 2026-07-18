use actix_web::{web, HttpResponse, Responder};
use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::pathutil::{resolve_within_root, PathError};
use crate::AppState;

#[derive(Serialize)]
struct DirEntryInfo {
    name: String,
    /// URL path (relative to the API root) that can be used to browse into
    /// this subdirectory, e.g. "albums/2020".
    path: String,
}

#[derive(Serialize)]
struct FileEntryInfo {
    name: String,
    /// Size in bytes.
    size: u64,
    /// URL that can be used to GET the raw file content.
    url: String,
}

#[derive(Serialize)]
struct BrowseResponse {
    /// The directory path that was browsed, relative to the music root.
    /// Empty string for the root itself.
    path: String,
    directories: Vec<DirEntryInfo>,
    files: Vec<FileEntryInfo>,
}

/// Returns true for dotfiles / dot-directories (hidden entries) that we
/// don't want to expose in listings.
fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn list_directory(dir: &Path, rel_prefix: &str) -> std::io::Result<BrowseResponse> {
    let mut directories = Vec::new();
    let mut files = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy().to_string();

        if is_hidden(&name) {
            continue;
        }

        let file_type = entry.file_type()?;
        let entry_rel = if rel_prefix.is_empty() {
            name.clone()
        } else {
            format!("{rel_prefix}/{name}")
        };

        if file_type.is_dir() {
            directories.push(DirEntryInfo {
                name,
                path: entry_rel,
            });
        } else if file_type.is_file() {
            let size = entry.metadata()?.len();
            files.push(FileEntryInfo {
                name,
                size,
                url: format!("/api/files/{entry_rel}"),
            });
        }
        // Symlinks and other special file types are silently skipped.
    }

    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(BrowseResponse {
        path: rel_prefix.to_string(),
        directories,
        files,
    })
}

/// GET /api/browse
///
/// Lists the contents of the music root directory.
pub async fn browse_root(data: web::Data<AppState>) -> impl Responder {
    browse_path_impl(&data, "")
}

/// GET /api/browse/{path:.*}
///
/// Lists the contents of a subdirectory of the music root, given by `path`.
pub async fn browse_subpath(
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    browse_path_impl(&data, &path.into_inner())
}

fn browse_path_impl(data: &web::Data<AppState>, rel: &str) -> HttpResponse {
    let resolved = if rel.is_empty() {
        // Root is always valid (and already canonical) since we built the
        // server state from it.
        data.root.clone()
    } else {
        match resolve_within_root(&data.root, rel) {
            Ok(p) => p,
            Err(PathError::Traversal) => {
                return HttpResponse::BadRequest().json(serde_json::json!({
                    "error": "invalid path"
                }))
            }
            Err(PathError::NotFound) => {
                return HttpResponse::NotFound().json(serde_json::json!({
                    "error": "not found"
                }))
            }
            Err(PathError::Io(e)) => {
                log::error!("io error resolving {rel}: {e}");
                return HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": "internal error"
                }));
            }
        }
    };

    if !resolved.is_dir() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": "not a directory"
        }));
    }

    match list_directory(&resolved, rel.trim_matches('/')) {
        Ok(listing) => HttpResponse::Ok().json(listing),
        Err(e) => {
            log::error!("failed to list directory {resolved:?}: {e}");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "internal error"
            }))
        }
    }
}

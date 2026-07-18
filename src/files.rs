use actix_files::NamedFile;
use actix_web::{web, HttpRequest, HttpResponse, Responder};

use crate::pathutil::{resolve_within_root, PathError};
use crate::AppState;

/// GET /api/files/{path:.*}
///
/// Serves the raw contents of a file. Any filetype is allowed;
/// this endpoint just streams whatever bytes are found there.
///
/// Uses `actix_files::NamedFile`, which handles `Range` requests, conditional
/// GETs (`If-Modified-Since`/`ETag`), and correct `Content-Type` guessing via
/// `mime_guess` automatically - important for audio players that seek within
/// a track.
pub async fn get_file(
    req: HttpRequest,
    data: web::Data<AppState>,
    path: web::Path<String>,
) -> impl Responder {
    let rel = path.into_inner();

    let resolved = match resolve_within_root(&data.root, &rel) {
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
    };

    if !resolved.is_file() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": "not a file"
        }));
    }

    match NamedFile::open_async(&resolved).await {
        Ok(named_file) => named_file
            .use_last_modified(true)
            .into_response(&req),
        Err(e) => {
            log::error!("failed to open file {resolved:?}: {e}");
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "internal error"
            }))
        }
    }
}

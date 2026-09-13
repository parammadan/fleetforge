//! Serving the built interface from the binary.
//!
//! The leadership demonstration must be one command. Two terminals and a Vite
//! dev server is a demonstration with two ways to fail in front of an audience,
//! and the dev server is the wrong thing to show anyway — it is not what would
//! ever run.
//!
//! So `--ui <dir>` mounts a production build next to the API on the same port.
//! One process, one URL, one thing to stop.

use std::path::{Path, PathBuf};

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

/// Why a directory was rejected as a UI build.
#[derive(Debug, thiserror::Error)]
pub enum UiError {
    /// The directory does not exist.
    #[error("no interface build at {path} — run `npm run build` in web/ first")]
    Missing {
        /// The path that was checked.
        path: String,
    },
    /// The directory exists but holds no `index.html`.
    #[error("{path} exists but contains no index.html; it is not an interface build")]
    NotABuild {
        /// The path that was checked.
        path: String,
    },
}

/// Validate a UI directory before the server starts.
///
/// Checked up front rather than on the first request: a process that starts
/// happily and then serves 404 to the room is worse than one that refuses to
/// start and says why.
///
/// # Errors
///
/// Returns [`UiError`] if the directory is missing or holds no `index.html`.
pub fn validate(dir: &Path) -> Result<PathBuf, UiError> {
    if !dir.is_dir() {
        return Err(UiError::Missing {
            path: dir.display().to_string(),
        });
    }
    if !dir.join("index.html").is_file() {
        return Err(UiError::NotABuild {
            path: dir.display().to_string(),
        });
    }
    Ok(dir.to_path_buf())
}

/// Mount a built interface at `/`.
///
/// `ServeDir` resolves paths against the root and rejects anything that escapes
/// it, so the same traversal guarantee the artifact endpoint makes by using a
/// manifest applies here by construction.
///
/// Unknown paths fall back to `index.html` rather than 404, because the
/// interface is a single page: a reload on any route has to return the app.
/// Unknown paths under `/api/` are not routed here at all — those are matched
/// first and answer for themselves.
pub fn mount(router: Router, dir: &Path) -> Router {
    let index = dir.join("index.html");
    let serve = ServeDir::new(dir)
        .append_index_html_on_directories(true)
        .fallback(ServeFile::new(index));

    router.fallback_service(serve).layer(
        // The interface is served from the same origin as the API and holds no
        // credentials, but it does render captured cluster data. These are the
        // cheap headers that cost nothing and remove whole classes of mistake.
        SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ),
    )
}

/// What to say when `--ui` was not given and someone opens the root anyway.
pub async fn no_ui() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "FleetForge is running its API only.\n\n\
         The interface was not mounted: start with --ui web/dist, or run `make demo`.\n\
         The API is at /api/v1/ and /healthz.\n",
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_is_rejected_with_the_command_that_fixes_it() {
        let err = validate(Path::new("/nonexistent/ui")).unwrap_err();
        assert!(matches!(err, UiError::Missing { .. }));
        assert!(format!("{err}").contains("npm run build"));
    }

    #[test]
    fn a_directory_without_index_html_is_not_a_build() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("stray.txt"), "hi").unwrap();
        let err = validate(dir.path()).unwrap_err();
        assert!(matches!(err, UiError::NotABuild { .. }));
    }

    #[test]
    fn a_real_build_validates() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.html"), "<!doctype html>").unwrap();
        assert!(validate(dir.path()).is_ok());
    }
}

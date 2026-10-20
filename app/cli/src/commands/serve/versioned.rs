//! A shell file named by its build: `/b/<build>/<file>` is `dist`'s `<file>` while this box
//! serves `<build>`, and nothing once it serves another. `/b/current` names the build served,
//! so the page's loader learns it beside the files, whether or not the API answers.
//!
//! A page loads only its own build's URLs, and so does every worker it starts, so no page runs
//! two builds' files: a file of a build this box no longer serves is a 404, never the new
//! build's file under the old name, and so is a build's file asked for by no build's name. The
//! service worker keeps each build's files under these names, so a page on an older build is
//! served from that build's cache. Only the page and what names no build pass as they are.

use axum::extract::{Request, State as Extract};
use axum::http::{HeaderValue, StatusCode, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::State;

/// The prefix of every path named by a build.
pub const PREFIX: &str = "/b/";

/// The path that names the build this box serves.
pub const CURRENT: &str = "/b/current";

/// The paths no build names, each build's the same name: the page, its loader, the worker's
/// script, the manifest, the icons and the Android app's link file.
pub const ROOT: [&str; 9] = [
    "/",
    "/index.html",
    "/boot.js",
    "/sw.js",
    "/manifest.webmanifest",
    "/icon.svg",
    "/icon-192.png",
    "/icon-512.png",
    "/.well-known/assetlinks.json",
];

/// What a request's path is to the frontend.
#[derive(Debug, PartialEq, Eq)]
pub enum Path {
    /// A path no build names, served as it is.
    Root,
    /// A path under [`PREFIX`], read against the build served ([`named`]).
    Named,
    /// Any other path: a build's file asked for by no build's name.
    Unnamed,
}

/// A path under [`PREFIX`], once the build served is known.
#[derive(Debug, PartialEq, Eq)]
pub enum Named<'a> {
    /// A file of the build this box serves, as `dist` names it (with its leading `/`).
    Own(&'a str),
    /// A file of another build, or no file.
    Other,
    /// The name of the build served.
    Current,
}

/// Reads `path` with no build: only a path under [`PREFIX`] needs one.
#[must_use]
pub fn read(path: &str) -> Path {
    if ROOT.contains(&path) {
        Path::Root
    } else if path.starts_with(PREFIX) {
        Path::Named
    } else {
        Path::Unnamed
    }
}

/// Reads a path under [`PREFIX`] against the build this box serves.
#[must_use]
pub fn named<'a>(path: &'a str, build: &str) -> Named<'a> {
    if path == CURRENT {
        return Named::Current;
    }
    let rest = path.strip_prefix(PREFIX).unwrap_or_default();
    match rest.split_once('/') {
        Some((b, file)) if b == build && !file.is_empty() => {
            Named::Own(&path[PREFIX.len() + b.len()..])
        }
        _ => Named::Other,
    }
}

/// Serves an own build's file under its plain name, and refuses another build's and a build's
/// file under no build's name. The build served is read once, and only for a path that names one.
pub async fn serve(Extract(state): Extract<State>, mut request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    match read(&path) {
        Path::Root => return next.run(request).await,
        Path::Unnamed => return StatusCode::NOT_FOUND.into_response(),
        Path::Named => {}
    }
    let build = state.stamp.build();
    match named(&path, &build) {
        Named::Other => StatusCode::NOT_FOUND.into_response(),
        Named::Current => {
            let mut answer = build.into_response();
            answer
                .headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
            answer
        }
        Named::Own(file) => {
            let query = request
                .uri()
                .query()
                .map_or_else(String::new, |q| format!("?{q}"));
            match format!("{file}{query}").parse::<Uri>() {
                Ok(uri) => {
                    *request.uri_mut() = uri;
                    next.run(request).await
                }
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Named, Path, named, read};

    #[test]
    fn a_file_of_the_build_served_is_its_plain_file() {
        assert_eq!(
            named("/b/7crcansr/app.js", "7crcansr"),
            Named::Own("/app.js")
        );
        assert_eq!(
            named("/b/7crcansr/typst/kit.typ", "7crcansr"),
            Named::Own("/typst/kit.typ")
        );
        assert_eq!(named("/b/dev/app.js", "dev"), Named::Own("/app.js"));
    }

    #[test]
    fn a_file_of_another_build_is_refused_never_served_as_this_one() {
        assert_eq!(named("/b/older/app.js", "7crcansr"), Named::Other);
        assert_eq!(named("/b/7crcansr/", "7crcansr"), Named::Other);
        assert_eq!(named("/b/7crcansr", "7crcansr"), Named::Other);
        assert_eq!(named("/b/", "7crcansr"), Named::Other);
    }

    #[test]
    fn current_names_the_build_served_whatever_it_is() {
        assert_eq!(named("/b/current", "7crcansr"), Named::Current);
        assert_eq!(named("/b/current", "current"), Named::Current);
    }

    #[test]
    fn only_a_path_under_the_prefix_needs_the_build() {
        assert_eq!(read("/b/current"), Path::Named);
        assert_eq!(read("/b/7crcansr/app.js"), Path::Named);
        for root in [
            "/",
            "/index.html",
            "/boot.js",
            "/sw.js",
            "/.well-known/assetlinks.json",
        ] {
            assert_eq!(read(root), Path::Root, "{root}");
        }
    }

    #[test]
    fn a_builds_file_under_no_builds_name_is_refused() {
        for plain in [
            "/app.js",
            "/style.css",
            "/tokens.css",
            "/prodrome_bg.wasm",
            "/typst/kit.typ",
            "/fonts/x.woff2",
            "/bundle/app.js",
        ] {
            assert_eq!(read(plain), Path::Unnamed, "{plain}");
        }
    }
}

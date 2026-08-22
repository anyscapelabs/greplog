//! The embedded dashboard: `dashboard/dist` served from inside the binary.
//! Debug builds read from disk so `npm run build` is picked up without a
//! `cargo` rebuild; `allow_missing` keeps a fresh checkout buildable, with the
//! handler naming the fix instead of failing silently.

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../dashboard/dist/"]
#[allow_missing = true]
struct Dashboard;

const INDEX: &str = "index.html";

/// Shown when the binary was built without a dashboard bundle.
const NOT_BUILT_HTML: &str = r#"<!doctype html>
<meta charset="utf-8">
<title>Greplog — dashboard not built</title>
<style>
  body { font: 15px/1.6 ui-sans-serif, system-ui, sans-serif; background: #09090b; color: #e4e4e7;
         display: grid; place-items: center; min-height: 100vh; margin: 0; }
  main { max-width: 34rem; padding: 2rem; }
  code { background: #18181b; border: 1px solid #27272a; border-radius: 6px; padding: .15em .4em; }
  pre  { background: #18181b; border: 1px solid #27272a; border-radius: 8px; padding: 1rem; overflow-x: auto; }
  a { color: #a06bff; }
</style>
<main>
  <h1>Dashboard not built</h1>
  <p>The API is running, but this binary carries no dashboard bundle.</p>
  <pre>cd dashboard &amp;&amp; npm install &amp;&amp; npm run build</pre>
  <p>Then restart <code>greplog dev</code>. The API itself is live at
     <code>POST /api/query</code> and <code>GET /api/tail</code>.</p>
</main>
"#;

/// Serves a dashboard asset, falling back to the SPA entrypoint so client-side
/// routes resolve through the browser router.
pub async fn handle_asset(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { INDEX } else { path };

    match Dashboard::get(path).or_else(|| Dashboard::get(INDEX)) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            (
                [
                    (header::CONTENT_TYPE, mime.as_ref().to_string()),
                    (header::CACHE_CONTROL, cache_control(path).to_string()),
                ],
                file.data,
            )
                .into_response()
        }
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            NOT_BUILT_HTML,
        )
            .into_response(),
    }
}

/// Vite fingerprints `assets/` (immutable, cache a year); `index.html` names
/// them and must always revalidate.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        return "public, max-age=31536000, immutable";
    }
    "no-cache"
}

#[cfg(test)]
mod tests {
    use super::cache_control;

    #[test]
    fn fingerprinted_assets_cache_forever_and_html_never_does() {
        assert!(cache_control("assets/index-a1b2c3.js").contains("immutable"));
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("favicon.svg"), "no-cache");
    }
}

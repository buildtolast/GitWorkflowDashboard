/// Embedded single-file HTML frontend.
/// Assembled from modular source files (styles/*.css, js/*.js, shell_*.html)
/// by build.rs at compile time.
pub const INDEX_HTML: &str = include_str!(concat!(env!("OUT_DIR"), "/index.html"));

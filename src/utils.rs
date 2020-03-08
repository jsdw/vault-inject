/// Take a base URL and an API path we wish to call, and return a URL
/// that is pointed at said API path.
pub fn make_api_path(mut url: url::Url, path: &str) -> url::Url {
    let path = format!(
        "{prefix}/v1/{path}",
        prefix = url.path().trim_matches('/'),
        path = path.trim_matches('/')
    );
    url.set_path(&path);
    url
}
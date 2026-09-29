use axum::http::{HeaderMap, StatusCode};

// Tokenless mode is loopback-only. Validate browser authority as well to reject
// DNS rebinding and cross-site requests, including simple requests without CORS.
pub(crate) fn check(headers: &HeaderMap) -> Result<(), StatusCode> {
    if ["forwarded", "x-forwarded-for", "x-forwarded-host"]
        .iter()
        .any(|key| headers.contains_key(*key))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::FORBIDDEN)?;
    fn local(authority: &str) -> bool {
        authority
            .parse::<axum::http::uri::Authority>()
            .ok()
            .is_some_and(|a| {
                let host = a.host().trim_matches(['[', ']']);
                host.eq_ignore_ascii_case("localhost")
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|ip| ip.is_loopback())
            })
    }
    if !local(host) {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(origin) = headers.get("origin") {
        let uri = origin
            .to_str()
            .ok()
            .and_then(|v| v.parse::<axum::http::Uri>().ok())
            .ok_or(StatusCode::FORBIDDEN)?;
        if !matches!(uri.scheme_str(), Some("http" | "https"))
            || !uri.authority().is_some_and(|a| local(a.as_str()))
        {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    if headers
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site")
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_local_browser_authorities_are_accepted() {
        let mut headers = HeaderMap::new();
        assert!(check(&headers).is_err());
        headers.insert("host", "127.0.0.1:8787".parse().unwrap());
        assert!(check(&headers).is_ok());
        headers.insert("origin", "https://evil.example".parse().unwrap());
        assert!(check(&headers).is_err());
        headers.insert("origin", "http://localhost:5173".parse().unwrap());
        assert!(check(&headers).is_ok());
        headers.insert("host", "rebinding.example:8787".parse().unwrap());
        assert!(check(&headers).is_err());
    }
}

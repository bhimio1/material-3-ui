//! A small blocking HTTP client layer over ureq: agents with timeouts and
//! a user agent, and error statuses as an [`HttpError`] value so callers can
//! react to 401/404/410/412.

use anyhow::{anyhow, Result};
use std::time::Duration;

/// A non-success HTTP status and the response body.
#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub body: String,
}

impl HttpError {
    /// "HTTP 404: not found", the body cut to 200 characters.
    pub fn summary(&self) -> String {
        let body: String = self.body.trim().chars().take(200).collect();
        if body.is_empty() {
            format!("HTTP {}", self.status)
        } else {
            format!("HTTP {}: {body}", self.status)
        }
    }
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.summary())
    }
}

impl std::error::Error for HttpError {}

/// The HTTP status behind an error, if it was one.
pub fn status_of(e: &anyhow::Error) -> Option<u16> {
    e.downcast_ref::<HttpError>().map(|h| h.status)
}

/// An agent with a 30 s timeout. `user_agent` is e.g.
/// `concat!("m3-calendar/", env!("CARGO_PKG_VERSION"))`.
pub fn agent(user_agent: &str, follow_redirects: bool) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .redirects(if follow_redirects { 5 } else { 0 })
        .user_agent(user_agent)
        .build()
}

fn result(res: std::result::Result<ureq::Response, ureq::Error>) -> Result<ureq::Response> {
    match res {
        Ok(r) => Ok(r),
        Err(ureq::Error::Status(status, r)) => Err(HttpError { status, body: r.into_string().unwrap_or_default() }.into()),
        Err(e) => Err(anyhow!("{e}")),
    }
}

/// Send a request (with a string body, or none), turning error statuses
/// into [`HttpError`].
pub fn send(req: ureq::Request, body: Option<&str>) -> Result<ureq::Response> {
    result(match body {
        Some(b) => req.send_string(b),
        None => req.call(),
    })
}

/// POST an `application/x-www-form-urlencoded` form and return the body.
pub fn form_post(agent: &ureq::Agent, url: &str, fields: &[(&str, String)]) -> Result<String> {
    let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    Ok(result(agent.post(url).send_form(&pairs))?.into_string()?)
}

/// Percent-encode one URL path segment (ids with '@', '#' or spaces).
pub fn path_segment(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>().replace('+', "%20")
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::testing::MockServer;

    #[test]
    fn statuses_become_errors() {
        let server = MockServer::start(|req| match req.path.as_str() {
            "/ok" => (200u16, "fine"),
            "/gone" => (410, "gone for good"),
            _ => (404, ""),
        });
        let a = agent("m3-test/1", false);
        assert_eq!(send(a.get(&format!("{}/ok", server.url)), None).unwrap().into_string().unwrap(), "fine");
        let e = send(a.get(&format!("{}/gone", server.url)), None).unwrap_err();
        assert_eq!(status_of(&e), Some(410));
        assert_eq!(e.to_string(), "HTTP 410: gone for good");
        let e = send(a.put(&format!("{}/x", server.url)), Some("body")).unwrap_err();
        assert_eq!(e.to_string(), "HTTP 404");
        assert_eq!(server.requests()[2].body, "body");
        assert_eq!(server.requests()[0].headers["user-agent"], "m3-test/1");
    }

    #[test]
    fn forms_are_encoded() {
        let server = MockServer::start(|req| (200u16, req.body.clone()));
        let body = form_post(&agent("t", true), &server.url, &[("a", "1 2".into()), ("b", "x&y".into())]).unwrap();
        assert_eq!(body, "a=1+2&b=x%26y");
        let req = &server.requests()[0];
        assert_eq!(req.form("b").as_deref(), Some("x&y"));
        assert_eq!(req.headers["content-type"], "application/x-www-form-urlencoded");
        assert_eq!(path_segment("a@b.com#x y"), "a%40b.com%23x%20y");
    }

    #[test]
    fn unreachable_servers_are_plain_errors() {
        let e = send(agent("t", false).get("http://127.0.0.1:9/"), None).unwrap_err();
        assert_eq!(status_of(&e), None);
    }
}

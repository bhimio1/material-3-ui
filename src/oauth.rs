//! OAuth 2.0 for installed apps (RFC 8252): the authorization code flow
//! with PKCE and a loopback redirect. The browser opens the provider's
//! sign-in page, which sends the code back to a one-shot server on
//! 127.0.0.1; the code is then traded for tokens. [`TokenSource`] keeps an
//! access token fresh and stores rotated refresh tokens in the keyring.
//!
//! ```no_run
//! use material_3_ui::oauth::{self, OAuthConfig};
//! # fn main() -> anyhow::Result<()> {
//! let cfg = OAuthConfig {
//!     auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
//!     token_url: "https://oauth2.googleapis.com/token".into(),
//!     client_id: "<client id>".into(),
//!     scopes: vec!["https://www.googleapis.com/auth/calendar".into()],
//!     app_name: "Calendar".into(),
//!     ..Default::default()
//! };
//! let tokens = oauth::sign_in(&cfg, oauth::open_browser, std::time::Duration::from_secs(300))?;
//! # Ok(())
//! # }
//! ```

use crate::http::{self, HttpError};
use crate::secrets::Secrets;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct OAuthConfig {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    /// Google's "Desktop app" clients come with a (non-confidential) secret
    /// that the token endpoint still asks for.
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    /// Extra authorization parameters (`prompt=consent`,
    /// `access_type=offline`, …).
    pub extra: Vec<(String, String)>,
    /// "127.0.0.1" (Google) or "localhost" (Microsoft only accepts the
    /// registered `http://localhost`, with any port).
    pub redirect_host: String,
    /// Named on the "you can close this tab" page.
    pub app_name: String,
    /// Send the scopes again when refreshing (Microsoft wants them).
    /// Defaults to doing so for clients without a secret.
    pub scopes_on_refresh: Option<bool>,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            auth_url: String::new(),
            token_url: String::new(),
            client_id: String::new(),
            client_secret: None,
            scopes: Vec::new(),
            extra: Vec::new(),
            redirect_host: "127.0.0.1".into(),
            app_name: "Material 3".into(),
            scopes_on_refresh: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Unix time, a minute early.
    pub expires_at: i64,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
}

const UNRESERVED: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";

/// Random characters from the URL-safe "unreserved" set.
pub fn random_string(len: usize) -> String {
    (0..len).map(|_| UNRESERVED[fastrand::usize(..UNRESERVED.len())] as char).collect()
}

/// A fresh (verifier, S256 challenge) pair.
pub fn pkce_pair() -> (String, String) {
    let verifier = random_string(64);
    let challenge = challenge_for(&verifier);
    (verifier, challenge)
}

/// BASE64URL(SHA256(verifier)) without padding.
pub fn challenge_for(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// The provider's sign-in page for this request.
pub fn authorize_url(cfg: &OAuthConfig, redirect: &str, challenge: &str, state: &str) -> Result<String> {
    let mut url = url::Url::parse(&cfg.auth_url).with_context(|| format!("bad authorization URL {}", cfg.auth_url))?;
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", &cfg.client_id)
            .append_pair("redirect_uri", redirect)
            .append_pair("response_type", "code")
            .append_pair("scope", &cfg.scopes.join(" "))
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", state);
        for (k, v) in &cfg.extra {
            q.append_pair(k, v);
        }
    }
    Ok(url.into())
}

/// Open a URL in the default browser (`xdg-open`), for [`sign_in`].
pub fn open_browser(url: &str) -> Result<()> {
    crate::sys::cmd::spawn_detached("xdg-open", &[url]).map_err(|e| anyhow!("couldn't open the browser: {e}"))
}

/// The whole sign-in: listens on a loopback port, opens the provider's
/// page with `open`, waits (up to `timeout`) for the redirect, then trades
/// the code for tokens. Blocking; run it on a background thread.
pub fn sign_in(cfg: &OAuthConfig, open: impl FnOnce(&str) -> Result<()>, timeout: Duration) -> Result<Tokens> {
    let listener = TcpListener::bind("127.0.0.1:0").context("starting the sign-in listener")?;
    let redirect = format!("http://{}:{}", cfg.redirect_host, listener.local_addr()?.port());
    let (verifier, challenge) = pkce_pair();
    let state = random_string(24);
    open(&authorize_url(cfg, &redirect, &challenge, &state)?)?;
    let code = wait_for_code(&listener, &state, &cfg.app_name, timeout)?;
    exchange_code(cfg, &code, &verifier, &redirect)
}

/// Serve one redirect on `listener` and return its authorization code.
/// Requests without a result (the browser's /favicon.ico) are answered
/// with 404 and ignored.
pub fn wait_for_code(listener: &TcpListener, state: &str, app_name: &str, timeout: Duration) -> Result<String> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    loop {
        let (mut stream, _) = match listener.accept() {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    bail!("sign-in timed out");
                }
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut line = String::new();
        if BufReader::new(&stream).read_line(&mut line).is_err() {
            continue;
        }
        let target = line.split_whitespace().nth(1).unwrap_or("/");
        let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else {
            continue;
        };
        let get = |k: &str| url.query_pairs().find(|(key, _)| key == k).map(|(_, v)| v.into_owned());
        if get("code").is_none() && get("error").is_none() {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            continue;
        }
        let (ok, result) = match (get("state"), get("code"), get("error")) {
            (_, _, Some(err)) => (false, Err(anyhow!("sign-in was cancelled ({err})"))),
            (Some(s), Some(code), None) if s == state => (true, Ok(code)),
            _ => (false, Err(anyhow!("the sign-in response didn't match this request"))),
        };
        let body = format!(
            "<!doctype html><meta charset=utf-8><title>{app_name}</title>\
             <body style=\"font-family:sans-serif;display:grid;place-items:center;height:90vh;background:#1b1b1f;color:#e4e1e6\">\
             <div style=\"text-align:center\"><h1>{}</h1><p>You can close this tab and go back to {app_name}.</p></div>",
            if ok { "Signed in" } else { "Sign-in didn't finish" }
        );
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        return result;
    }
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn token_request(cfg: &OAuthConfig, mut fields: Vec<(&str, String)>) -> Result<Tokens> {
    fields.push(("client_id", cfg.client_id.clone()));
    if let Some(secret) = &cfg.client_secret {
        fields.push(("client_secret", secret.clone()));
    }
    let agent = http::agent(concat!("material-3-ui/", env!("CARGO_PKG_VERSION")), true);
    let body = http::form_post(&agent, &cfg.token_url, &fields).map_err(|e| match e.downcast_ref::<HttpError>() {
        Some(h) if h.status == 400 || h.status == 401 => {
            anyhow!("the provider refused the sign-in ({}); sign in again", h.summary())
        }
        _ => e,
    })?;
    let r: TokenResponse = serde_json::from_str(&body).context("reading the token response")?;
    Ok(Tokens {
        access_token: r.access_token,
        refresh_token: r.refresh_token,
        expires_at: now() + r.expires_in.unwrap_or(3600) - 60,
    })
}

/// Trade an authorization code for tokens.
pub fn exchange_code(cfg: &OAuthConfig, code: &str, verifier: &str, redirect: &str) -> Result<Tokens> {
    token_request(
        cfg,
        vec![
            ("grant_type", "authorization_code".into()),
            ("code", code.into()),
            ("code_verifier", verifier.into()),
            ("redirect_uri", redirect.into()),
        ],
    )
}

/// New tokens from a refresh token. The provider may rotate the refresh
/// token: keep the returned one when it's `Some`.
pub fn refresh(cfg: &OAuthConfig, refresh_token: &str) -> Result<Tokens> {
    let mut fields = vec![("grant_type", "refresh_token".to_string()), ("refresh_token", refresh_token.to_string())];
    if cfg.scopes_on_refresh.unwrap_or(cfg.client_secret.is_none()) {
        fields.push(("scope", cfg.scopes.join(" ")));
    }
    token_request(cfg, fields)
}

/// An access token kept fresh from a refresh token. A rotated refresh
/// token is written back to `secrets` under `key`.
pub struct TokenSource {
    cfg: Option<OAuthConfig>,
    refresh_token: String,
    store: Option<(Arc<dyn Secrets>, String, String)>,
    cached: Option<Tokens>,
}

impl TokenSource {
    /// `key` and `label` are the keyring entry holding the refresh token.
    pub fn new(cfg: OAuthConfig, refresh_token: String, secrets: Arc<dyn Secrets>, key: String, label: String) -> Self {
        Self { cfg: Some(cfg), refresh_token, store: Some((secrets, key, label)), cached: None }
    }

    /// A fixed token that never expires, for tests.
    pub fn fixed(token: &str) -> Self {
        Self {
            cfg: None,
            refresh_token: String::new(),
            store: None,
            cached: Some(Tokens { access_token: token.into(), refresh_token: None, expires_at: i64::MAX }),
        }
    }

    /// A valid access token, refreshed when it's about to expire.
    pub fn access_token(&mut self) -> Result<String> {
        if let Some(t) = self.cached.as_ref().filter(|t| t.expires_at > now()) {
            return Ok(t.access_token.clone());
        }
        let cfg = self.cfg.as_ref().ok_or_else(|| anyhow!("not signed in"))?;
        let tokens = refresh(cfg, &self.refresh_token)?;
        if let (Some(new), Some((secrets, key, label))) = (&tokens.refresh_token, &self.store) {
            if *new != self.refresh_token {
                secrets.set(key, label, new)?;
                self.refresh_token = new.clone();
            }
        }
        let token = tokens.access_token.clone();
        self.cached = Some(tokens);
        Ok(token)
    }

    /// "Bearer <token>", for an Authorization header.
    pub fn bearer(&mut self) -> Result<String> {
        Ok(format!("Bearer {}", self.access_token()?))
    }

    /// Forget the cached access token (after a 401), so the next call
    /// refreshes.
    pub fn invalidate(&mut self) {
        if self.cfg.is_some() {
            self.cached = None;
        }
    }
}

#[cfg(all(test, feature = "testing"))]
mod tests {
    use super::*;
    use crate::testing::MockServer;
    use std::collections::HashMap;

    fn config(token_url: String) -> OAuthConfig {
        OAuthConfig {
            auth_url: "https://auth.example/authorize".into(),
            token_url,
            client_id: "cid".into(),
            scopes: vec!["a".into(), "b".into()],
            extra: vec![("prompt".into(), "consent".into())],
            app_name: "Test".into(),
            ..Default::default()
        }
    }

    fn query(url: &str) -> HashMap<String, String> {
        url::Url::parse(url).unwrap().query_pairs().into_owned().collect()
    }

    #[test]
    fn pkce_challenge_is_s256() {
        // The example from RFC 7636, appendix B.
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let (v, c) = pkce_pair();
        assert_eq!(v.len(), 64);
        assert!(v.bytes().all(|b| UNRESERVED.contains(&b)));
        assert_eq!(challenge_for(&v), c);
    }

    #[test]
    fn authorize_url_has_pkce_fields() {
        let q = query(&authorize_url(&config(String::new()), "http://127.0.0.1:5000", "chal", "st").unwrap());
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], "chal");
        assert_eq!(q["scope"], "a b");
        assert_eq!(q["redirect_uri"], "http://127.0.0.1:5000");
        assert_eq!(q["response_type"], "code");
        assert_eq!(q["prompt"], "consent");
        assert!(authorize_url(&OAuthConfig::default(), "x", "c", "s").is_err());
    }

    /// Play the browser: follow the redirect with `answer` as its query.
    fn browser(answer: impl Fn(&HashMap<String, String>) -> String + Send + 'static) -> impl FnOnce(&str) -> Result<()> {
        move |url| {
            let q = query(url);
            let back = format!("{}/?{}", q["redirect_uri"], answer(&q));
            let favicon = format!("{}/favicon.ico", q["redirect_uri"]);
            std::thread::spawn(move || {
                let _ = ureq::get(&favicon).call();
                let _ = ureq::get(&back).call();
            });
            Ok(())
        }
    }

    #[test]
    fn loopback_flow_end_to_end() {
        let server = MockServer::start(|req| {
            assert_eq!(req.method, "POST");
            assert_eq!(req.form("grant_type").as_deref(), Some("authorization_code"));
            assert_eq!(req.form("code").as_deref(), Some("the-code"));
            assert_eq!(req.form("client_id").as_deref(), Some("cid"));
            assert!(req.form("redirect_uri").unwrap().starts_with("http://127.0.0.1:"));
            (200u16, r#"{"access_token":"at","refresh_token":"rt","expires_in":3600}"#)
        });
        let cfg = config(format!("{}/token", server.url));
        let challenge = Arc::new(std::sync::Mutex::new(String::new()));
        let seen = challenge.clone();
        let tokens = sign_in(
            &cfg,
            browser(move |q| {
                *seen.lock().unwrap() = q["code_challenge"].clone();
                format!("code=the-code&state={}", q["state"])
            }),
            Duration::from_secs(10),
        )
        .unwrap();
        assert_eq!(tokens.access_token, "at");
        assert_eq!(tokens.refresh_token.as_deref(), Some("rt"));
        assert!(tokens.expires_at > now() + 3000);
        // The verifier sent with the code matches the challenge sent to the browser.
        let verifier = server.requests()[0].form("code_verifier").unwrap();
        assert_eq!(challenge_for(&verifier), *challenge.lock().unwrap());
    }

    #[test]
    fn mismatched_or_cancelled_sign_ins_fail() {
        let cfg = config("http://127.0.0.1:9/token".into());
        let e = sign_in(&cfg, browser(|_| "code=x&state=forged".into()), Duration::from_secs(10)).unwrap_err();
        assert!(e.to_string().contains("didn't match"), "{e}");
        let e = sign_in(&cfg, browser(|_| "error=access_denied".into()), Duration::from_secs(10)).unwrap_err();
        assert!(e.to_string().contains("cancelled (access_denied)"), "{e}");
        let e = sign_in(&cfg, |_| Ok(()), Duration::from_millis(200)).unwrap_err();
        assert!(e.to_string().contains("timed out"), "{e}");
    }

    #[test]
    fn refused_codes_say_to_sign_in_again() {
        let server = MockServer::start(|_| (400u16, r#"{"error":"invalid_grant"}"#));
        let e = exchange_code(&config(server.url.clone()), "c", "v", "r").unwrap_err();
        assert!(e.to_string().contains("sign in again"), "{e}");
        let server = MockServer::start(|_| (200u16, "not json"));
        assert!(exchange_code(&config(server.url.clone()), "c", "v", "r").is_err());
    }

    #[test]
    fn token_source_refreshes_and_rotates_the_stored_token() {
        let server = MockServer::start(|req| {
            assert_eq!(req.form("grant_type").as_deref(), Some("refresh_token"));
            assert_eq!(req.form("refresh_token").as_deref(), Some("old"));
            // No client secret: the scopes are sent again.
            assert_eq!(req.form("scope").as_deref(), Some("a b"));
            (200u16, r#"{"access_token":"fresh","refresh_token":"new","expires_in":3600}"#)
        });
        let secrets: Arc<dyn Secrets> = Arc::new(crate::secrets::Memory::default());
        let cfg = config(format!("{}/token", server.url));
        let mut ts = TokenSource::new(cfg, "old".into(), secrets.clone(), "account:x".into(), "label".into());
        assert_eq!(ts.bearer().unwrap(), "Bearer fresh");
        // Cached: no second request.
        assert_eq!(ts.access_token().unwrap(), "fresh");
        assert_eq!(server.requests().len(), 1);
        assert_eq!(secrets.get("account:x").unwrap().as_deref(), Some("new"));

        let mut fixed = TokenSource::fixed("tok");
        fixed.invalidate();
        assert_eq!(fixed.bearer().unwrap(), "Bearer tok");
    }
}

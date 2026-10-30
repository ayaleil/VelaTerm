//! Optional Cloudflare Access listener: a loopback-only plaintext instance meant to sit behind the user's own
//! `cloudflared` tunnel. VelaTerm never creates tunnels, Access applications, or DNS records; it only verifies
//! the Access JWT that Cloudflare attaches to every proxied request, before any route runs. A request without
//! a valid token is rejected, and so is every request while the team's signing keys cannot be fetched.
//!
//! Known limitation: an open WebSocket is not re-validated when its JWT expires; the next HTTP request or
//! reconnect is.

use std::collections::HashMap;
use std::io::Read;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use jsonwebtoken::errors::ErrorKind;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use serde::Deserialize;

use super::rate_limit::LimitKey;
use super::WebServer;

/// Default loopback port of the Cloudflare Access listener, next to the LAN default 8799.
pub(crate) const DEFAULT_PORT: u16 = 8800;
const TEAM_DOMAIN_SUFFIX: &str = ".cloudflareaccess.com";
const JWT_HEADER: &str = "cf-access-jwt-assertion";
const JWT_COOKIE: &str = "CF_Authorization";
/// A forged `kid` must not turn every request into a fetch against Cloudflare.
const REFETCH_INTERVAL: Duration = Duration::from_secs(60);
/// With no key cached every request fails, so a failed boot-time fetch is retried sooner.
const EMPTY_RETRY_INTERVAL: Duration = Duration::from_secs(10);
const LEEWAY_SECS: u64 = 30;
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
/// The real certs document is a few KB; anything far larger is not a key set worth buffering.
const MAX_CERTS_BYTES: u64 = 1024 * 1024;
/// Rejections are logged at most once per kind in this window, so a scanner cannot flood the log.
const REJECTION_LOG_INTERVAL: Duration = Duration::from_secs(30);

/// Tauri-managed state for the Cloudflare Access instance, kept apart from the LAN `WebServer` so either
/// can run alone or both together.
pub(crate) struct CloudflareAccessServer(pub(crate) WebServer);

impl CloudflareAccessServer {
    pub(crate) fn new() -> Self {
        Self(WebServer::new())
    }
}

impl Default for CloudflareAccessServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Validated Access settings: normalized team domain, AUD tag, and lowercase allowed emails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccessConfig {
    pub(crate) team_domain: String,
    pub(crate) aud: String,
    pub(crate) allowed_emails: Vec<String>,
}

impl AccessConfig {
    pub(crate) fn new(team_domain: &str, aud: &str, allowed_emails: &[String]) -> Result<Self, String> {
        let team_domain = normalize_team_domain(team_domain)?;
        let aud = aud.trim();
        if aud.is_empty() || aud.chars().any(char::is_whitespace) {
            return Err("Enter the Application Audience (AUD) tag".into());
        }
        Ok(Self {
            team_domain,
            aud: aud.to_string(),
            allowed_emails: normalize_emails(allowed_emails),
        })
    }

    fn issuer(&self) -> String {
        format!("https://{}", self.team_domain)
    }

    fn certs_url(&self) -> String {
        format!("https://{}/cdn-cgi/access/certs", self.team_domain)
    }
}

/// Accepts `myteam.cloudflareaccess.com` with or without scheme and trailing slash; anything that is not a
/// plain hostname under `cloudflareaccess.com` is rejected, so the certs URL can never point elsewhere.
fn normalize_team_domain(raw: &str) -> Result<String, String> {
    let lower = raw.trim().to_ascii_lowercase();
    let host = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower)
        .trim_end_matches('/');
    let valid = host.len() > TEAM_DOMAIN_SUFFIX.len()
        && host.ends_with(TEAM_DOMAIN_SUFFIX)
        && host
            .split('.')
            .all(|label| !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
    if !valid {
        return Err("Team domain must look like <team>.cloudflareaccess.com".into());
    }
    Ok(host.to_string())
}

/// Trims, lowercases, and drops empty or duplicate entries; matching against the `email` claim is
/// case-insensitive.
pub(crate) fn normalize_emails(raw: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for email in raw {
        let email = email.trim().to_ascii_lowercase();
        if !email.is_empty() && !out.contains(&email) {
            out.push(email);
        }
    }
    out
}

/// One entry of the Access certs document; extra members such as `public_cert` are ignored.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Jwk {
    kid: String,
    #[serde(default)]
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

/// Why a key fetch failed; a fixed category so the diagnostics log never carries response content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FetchError {
    Dns,
    Connect,
    Io,
    Status,
    TooLarge,
    Parse,
    Other,
}

/// Where signing keys come from; injectable so tests never touch the network.
pub(crate) trait KeySource: Send + Sync {
    fn fetch(&self) -> Result<Vec<Jwk>, FetchError>;
}

struct HttpKeySource {
    url: String,
}

impl KeySource for HttpKeySource {
    fn fetch(&self) -> Result<Vec<Jwk>, FetchError> {
        // No redirects: the certs URL is pinned to the team domain and must not be bounced elsewhere.
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(FETCH_TIMEOUT)
            .build();
        let response = agent.get(&self.url).call().map_err(|e| match e {
            ureq::Error::Status(..) => FetchError::Status,
            ureq::Error::Transport(t) => match t.kind() {
                ureq::ErrorKind::Dns => FetchError::Dns,
                ureq::ErrorKind::ConnectionFailed => FetchError::Connect,
                ureq::ErrorKind::Io => FetchError::Io,
                _ => FetchError::Other,
            },
        })?;
        if response.status() != 200 {
            return Err(FetchError::Status);
        }
        let mut body = Vec::new();
        response
            .into_reader()
            .take(MAX_CERTS_BYTES + 1)
            .read_to_end(&mut body)
            .map_err(|_| FetchError::Io)?;
        if body.len() as u64 > MAX_CERTS_BYTES {
            return Err(FetchError::TooLarge);
        }
        let jwks: Jwks = serde_json::from_slice(&body).map_err(|_| FetchError::Parse)?;
        Ok(jwks.keys)
    }
}

fn log_fetch_error(error: FetchError) {
    match error {
        FetchError::Dns => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: DNS lookup"),
        FetchError::Connect => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: connection or TLS"),
        FetchError::Io => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: I/O or timeout"),
        FetchError::Status => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: non-200 response"),
        FetchError::TooLarge => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: response over 1 MB"),
        FetchError::Parse => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: invalid JWKS JSON"),
        FetchError::Other => crate::diagnostic_warn!("[cloudflare-access] key fetch failed: request error"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Rejection {
    MissingToken,
    Malformed,
    UnsupportedAlgorithm,
    MissingKid,
    UnknownKey,
    KeysUnavailable,
    InvalidSignature,
    InvalidAudience,
    InvalidIssuer,
    Expired,
    NotYetValid,
    Invalid,
    EmailNotAllowed,
}

impl Rejection {
    fn from_jwt(error: &jsonwebtoken::errors::Error) -> Self {
        match error.kind() {
            ErrorKind::InvalidSignature => Rejection::InvalidSignature,
            ErrorKind::InvalidAudience => Rejection::InvalidAudience,
            ErrorKind::InvalidIssuer => Rejection::InvalidIssuer,
            ErrorKind::ExpiredSignature => Rejection::Expired,
            ErrorKind::ImmatureSignature => Rejection::NotYetValid,
            _ => Rejection::Invalid,
        }
    }

    fn log(self) {
        match self {
            Rejection::MissingToken => crate::diagnostic_warn!("[cloudflare-access] rejected: no Access token"),
            Rejection::Malformed => crate::diagnostic_warn!("[cloudflare-access] rejected: malformed token"),
            Rejection::UnsupportedAlgorithm => crate::diagnostic_warn!("[cloudflare-access] rejected: algorithm is not RS256"),
            Rejection::MissingKid => crate::diagnostic_warn!("[cloudflare-access] rejected: token has no kid"),
            Rejection::UnknownKey => crate::diagnostic_warn!("[cloudflare-access] rejected: unknown signing key"),
            Rejection::KeysUnavailable => crate::diagnostic_warn!("[cloudflare-access] rejected: signing keys unavailable"),
            Rejection::InvalidSignature => crate::diagnostic_warn!("[cloudflare-access] rejected: invalid signature"),
            Rejection::InvalidAudience => crate::diagnostic_warn!("[cloudflare-access] rejected: audience mismatch"),
            Rejection::InvalidIssuer => crate::diagnostic_warn!("[cloudflare-access] rejected: issuer mismatch"),
            Rejection::Expired => crate::diagnostic_warn!("[cloudflare-access] rejected: token expired"),
            Rejection::NotYetValid => crate::diagnostic_warn!("[cloudflare-access] rejected: token not yet valid"),
            Rejection::Invalid => crate::diagnostic_warn!("[cloudflare-access] rejected: invalid claims"),
            Rejection::EmailNotAllowed => crate::diagnostic_warn!("[cloudflare-access] rejected: email not allowed"),
        }
    }
}

#[derive(Deserialize)]
struct Claims {
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sub: Option<String>,
    /// Set instead of `email` for Access service tokens.
    #[serde(default)]
    common_name: Option<String>,
}

/// Who a verified Access JWT belongs to. Only [`gate`] inserts it into request extensions, and only after
/// verification succeeded, so handlers can trust it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AccessIdentity(pub(crate) String);

/// Login attempts behind the tunnel are counted per verified Access identity; elsewhere per peer IP.
pub(crate) fn login_limit_key(ip: IpAddr, identity: Option<&AccessIdentity>) -> LimitKey {
    match identity {
        Some(AccessIdentity(id)) => LimitKey::Identity(id.clone()),
        None => LimitKey::Ip(ip),
    }
}

#[derive(Default)]
struct KeyCache {
    keys: HashMap<String, DecodingKey>,
    last_fetch: Option<Instant>,
}

impl KeyCache {
    fn miss(&self) -> Rejection {
        if self.keys.is_empty() {
            Rejection::KeysUnavailable
        } else {
            Rejection::UnknownKey
        }
    }
}

/// Clears the single-flight flag even if the fetch panics, so the gate cannot wedge shut.
struct InFlight<'a>(&'a AtomicBool);

impl Drop for InFlight<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// Verifies Access JWTs against the team's cached signing keys.
pub(crate) struct Verifier {
    config: AccessConfig,
    issuer: String,
    source: Box<dyn KeySource>,
    cache: Mutex<KeyCache>,
    fetching: AtomicBool,
    refetch_interval: Duration,
    empty_retry_interval: Duration,
    logged: Mutex<HashMap<Rejection, Instant>>,
}

impl Verifier {
    fn new(config: AccessConfig, source: Box<dyn KeySource>) -> Self {
        Self {
            issuer: config.issuer(),
            config,
            source,
            cache: Mutex::new(KeyCache::default()),
            fetching: AtomicBool::new(false),
            refetch_interval: REFETCH_INTERVAL,
            empty_retry_interval: EMPTY_RETRY_INTERVAL,
            logged: Mutex::new(HashMap::new()),
        }
    }

    /// Verifier fetching keys from `https://<team>/cdn-cgi/access/certs`.
    pub(crate) fn for_team(config: AccessConfig) -> Self {
        let source = HttpKeySource {
            url: config.certs_url(),
        };
        Self::new(config, Box::new(source))
    }

    pub(crate) fn verify_headers(&self, headers: &HeaderMap) -> Result<AccessIdentity, Rejection> {
        let token = token_from_headers(headers).ok_or(Rejection::MissingToken)?;
        self.verify(&token)
    }

    fn verify(&self, token: &str) -> Result<AccessIdentity, Rejection> {
        let header = jsonwebtoken::decode_header(token).map_err(|_| Rejection::Malformed)?;
        if header.alg != Algorithm::RS256 {
            return Err(Rejection::UnsupportedAlgorithm);
        }
        let kid = header.kid.ok_or(Rejection::MissingKid)?;
        let key = self.key_for(&kid)?;
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[self.config.aud.as_str()]);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud"]);
        validation.validate_nbf = true;
        validation.leeway = LEEWAY_SECS;
        let claims = jsonwebtoken::decode::<Claims>(token, &key, &validation)
            .map_err(|e| Rejection::from_jwt(&e))?
            .claims;
        let email = claims
            .email
            .map(|e| e.trim().to_ascii_lowercase())
            .filter(|e| !e.is_empty());
        if !self.config.allowed_emails.is_empty()
            && !email.as_ref().is_some_and(|e| self.config.allowed_emails.contains(e))
        {
            return Err(Rejection::EmailNotAllowed);
        }
        let identity = email
            .or(claims.sub.filter(|s| !s.is_empty()))
            .or(claims.common_name.filter(|s| !s.is_empty()))
            .unwrap_or_default();
        Ok(AccessIdentity(identity))
    }

    fn lock_cache(&self) -> MutexGuard<'_, KeyCache> {
        self.cache.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Returns the cached key for `kid`, refetching the key set when `kid` is unknown and the last fetch is
    /// old enough. The lock is never held across the fetch, so cache hits never wait on the network, and
    /// only one fetch runs at a time; concurrent misses are rejected instead of queueing behind it.
    fn key_for(&self, kid: &str) -> Result<DecodingKey, Rejection> {
        let _in_flight = {
            let mut cache = self.lock_cache();
            if let Some(key) = cache.keys.get(kid) {
                return Ok(key.clone());
            }
            let interval = if cache.keys.is_empty() {
                self.empty_retry_interval
            } else {
                self.refetch_interval
            };
            let due = cache.last_fetch.is_none_or(|at| at.elapsed() >= interval);
            if !due || self.fetching.swap(true, Ordering::AcqRel) {
                return Err(cache.miss());
            }
            cache.last_fetch = Some(Instant::now());
            InFlight(&self.fetching)
        };
        let fetched = self.source.fetch();
        let mut cache = self.lock_cache();
        match fetched {
            Ok(jwks) => {
                let keys = decoding_keys(jwks);
                if !keys.is_empty() {
                    cache.keys = keys;
                } else if cache.keys.is_empty() {
                    crate::diagnostic_warn!("[cloudflare-access] key set has no usable RSA keys");
                } else {
                    crate::diagnostic_warn!("[cloudflare-access] key set has no usable RSA keys; keeping cached keys");
                }
            }
            Err(e) => log_fetch_error(e),
        }
        cache.keys.get(kid).cloned().ok_or_else(|| cache.miss())
    }

    /// Whether this rejection kind should be logged now; at most once per kind per interval.
    fn should_log(&self, kind: Rejection, now: Instant) -> bool {
        let mut logged = self.logged.lock().unwrap_or_else(|p| p.into_inner());
        if logged
            .get(&kind)
            .is_some_and(|at| now.duration_since(*at) < REJECTION_LOG_INTERVAL)
        {
            return false;
        }
        logged.insert(kind, now);
        true
    }

    fn note_rejection(&self, kind: Rejection) {
        if self.should_log(kind, Instant::now()) {
            kind.log();
        }
    }
}

fn decoding_keys(jwks: Vec<Jwk>) -> HashMap<String, DecodingKey> {
    jwks.into_iter()
        .filter(|k| k.kty == "RSA")
        .filter_map(|k| {
            let key = DecodingKey::from_rsa_components(k.n.as_deref()?, k.e.as_deref()?).ok()?;
            Some((k.kid, key))
        })
        .collect()
}

/// The `Cf-Access-Jwt-Assertion` header wins; browsers that reach the origin directly over the tunnel
/// still carry the `CF_Authorization` cookie.
fn token_from_headers(headers: &HeaderMap) -> Option<String> {
    if let Some(token) = headers
        .get(JWT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        return Some(token.to_string());
    }
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().strip_prefix(JWT_COOKIE)?.strip_prefix('='))
        .map(str::trim)
        .find(|v| !v.is_empty())
        .map(str::to_string)
}

/// Outermost middleware of a Cloudflare Access instance; other instances carry `None` and pass through.
/// Verification runs on the blocking pool because a key refetch is a synchronous HTTP call. On success the
/// verified identity is attached to the request for the login rate limiter.
pub(crate) async fn gate(
    State(verifier): State<Option<Arc<Verifier>>>,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    if let Some(verifier) = verifier {
        let headers = request.headers().clone();
        let checker = verifier.clone();
        let verdict = tokio::task::spawn_blocking(move || checker.verify_headers(&headers))
            .await
            .unwrap_or(Err(Rejection::KeysUnavailable));
        match verdict {
            Ok(identity) => {
                request.extensions_mut().insert(identity);
            }
            Err(kind) => {
                verifier.note_rejection(kind);
                return (StatusCode::UNAUTHORIZED, "Cloudflare Access authorization required")
                    .into_response();
            }
        }
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::encoding::AsDer;
    use aws_lc_rs::signature::KeyPair as _;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::{get, post};
    use axum::{Extension, Router};
    use base64::Engine;
    use jsonwebtoken::{EncodingKey, Header};
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Barrier, OnceLock};
    use tower::ServiceExt;

    const TEAM: &str = "myteam.cloudflareaccess.com";
    const AUD: &str = "aud-tag-123";

    struct TestKey {
        encoding: EncodingKey,
        jwk: Jwk,
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn generate(kid: &str) -> TestKey {
        let pair = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
        let pkcs8 = pair.as_der().unwrap();
        let pem = pem::encode(&pem::Pem::new("PRIVATE KEY", pkcs8.as_ref().to_vec()));
        let public = pair.public_key();
        TestKey {
            encoding: EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(),
            jwk: Jwk {
                kid: kid.into(),
                kty: "RSA".into(),
                n: Some(b64(public.modulus().big_endian_without_leading_zero())),
                e: Some(b64(public.exponent().big_endian_without_leading_zero())),
            },
        }
    }

    /// RSA generation is slow, so the suite shares two keys.
    fn keys() -> &'static (TestKey, TestKey) {
        static KEYS: OnceLock<(TestKey, TestKey)> = OnceLock::new();
        KEYS.get_or_init(|| (generate("kid-a"), generate("kid-b")))
    }

    struct FakeSource {
        keys: Mutex<Result<Vec<Jwk>, FetchError>>,
        calls: AtomicUsize,
    }

    impl KeySource for Arc<FakeSource> {
        fn fetch(&self) -> Result<Vec<Jwk>, FetchError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.keys.lock().unwrap().clone()
        }
    }

    impl FakeSource {
        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn set(&self, keys: Result<Vec<Jwk>, FetchError>) {
            *self.keys.lock().unwrap() = keys;
        }
    }

    fn source(keys: Result<Vec<Jwk>, FetchError>) -> Arc<FakeSource> {
        Arc::new(FakeSource {
            keys: Mutex::new(keys),
            calls: AtomicUsize::new(0),
        })
    }

    /// The usual setup: a source serving only `kid-a`.
    fn kid_a_source() -> Arc<FakeSource> {
        source(Ok(vec![keys().0.jwk.clone()]))
    }

    fn verifier_with(source: &Arc<FakeSource>, emails: &[&str]) -> Verifier {
        let emails: Vec<String> = emails.iter().map(|e| e.to_string()).collect();
        let config = AccessConfig::new(TEAM, AUD, &emails).unwrap();
        Verifier::new(config, Box::new(source.clone()))
    }

    fn now() -> u64 {
        jsonwebtoken::get_current_timestamp()
    }

    fn sign(key: &TestKey, kid: &str, claims: serde_json::Value) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(kid.into());
        jsonwebtoken::encode(&header, &claims, &key.encoding).unwrap()
    }

    fn claims() -> serde_json::Value {
        serde_json::json!({
            "iss": format!("https://{TEAM}"),
            "aud": [AUD],
            "exp": now() + 600,
            "nbf": now() - 10,
            "iat": now() - 10,
            "email": "Dev@Example.com",
            "sub": "user-uuid",
        })
    }

    fn valid_token() -> String {
        sign(&keys().0, "kid-a", claims())
    }

    fn with_claim(key: &str, value: serde_json::Value) -> String {
        let mut c = claims();
        c[key] = value;
        sign(&keys().0, "kid-a", c)
    }

    #[test]
    fn valid_token_passes_with_email_identity() {
        let src = kid_a_source();
        assert_eq!(
            verifier_with(&src, &[]).verify(&valid_token()),
            Ok(AccessIdentity("dev@example.com".into()))
        );
    }

    #[test]
    fn identity_falls_back_to_sub_without_email() {
        let src = kid_a_source();
        let mut c = claims();
        c.as_object_mut().unwrap().remove("email");
        let token = sign(&keys().0, "kid-a", c);
        assert_eq!(
            verifier_with(&src, &[]).verify(&token),
            Ok(AccessIdentity("user-uuid".into()))
        );
    }

    #[test]
    fn bad_signature_is_rejected() {
        let src = kid_a_source();
        let forged = sign(&keys().1, "kid-a", claims());
        assert_eq!(verifier_with(&src, &[]).verify(&forged), Err(Rejection::InvalidSignature));
    }

    #[test]
    fn wrong_audience_is_rejected() {
        let src = kid_a_source();
        let token = with_claim("aud", serde_json::json!(["someone-else"]));
        assert_eq!(verifier_with(&src, &[]).verify(&token), Err(Rejection::InvalidAudience));
    }

    #[test]
    fn wrong_issuer_is_rejected() {
        let src = kid_a_source();
        let token = with_claim("iss", serde_json::json!("https://other.cloudflareaccess.com"));
        assert_eq!(verifier_with(&src, &[]).verify(&token), Err(Rejection::InvalidIssuer));
    }

    #[test]
    fn expired_token_is_rejected() {
        let src = kid_a_source();
        let token = with_claim("exp", serde_json::json!(now() - 3600));
        assert_eq!(verifier_with(&src, &[]).verify(&token), Err(Rejection::Expired));
    }

    #[test]
    fn future_nbf_is_rejected() {
        let src = kid_a_source();
        let token = with_claim("nbf", serde_json::json!(now() + 3600));
        assert_eq!(verifier_with(&src, &[]).verify(&token), Err(Rejection::NotYetValid));
    }

    #[test]
    fn alg_none_and_hs256_are_rejected() {
        let src = kid_a_source();
        let verifier = verifier_with(&src, &[]);
        let unsigned = format!(
            "{}.{}.",
            b64(br#"{"alg":"none","typ":"JWT","kid":"kid-a"}"#),
            b64(claims().to_string().as_bytes())
        );
        assert!(verifier.verify(&unsigned).is_err());

        let mut header = Header::new(Algorithm::HS256);
        header.kid = Some("kid-a".into());
        let hs256 = jsonwebtoken::encode(&header, &claims(), &EncodingKey::from_secret(b"secret")).unwrap();
        assert_eq!(verifier.verify(&hs256), Err(Rejection::UnsupportedAlgorithm));
        assert_eq!(src.calls(), 0, "no key fetch for a token that can never verify");
    }

    #[test]
    fn missing_kid_is_rejected() {
        let src = kid_a_source();
        let token = jsonwebtoken::encode(&Header::new(Algorithm::RS256), &claims(), &keys().0.encoding).unwrap();
        assert_eq!(verifier_with(&src, &[]).verify(&token), Err(Rejection::MissingKid));
    }

    #[test]
    fn missing_header_and_cookie_is_rejected() {
        let src = kid_a_source();
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "other=1".parse().unwrap());
        assert_eq!(
            verifier_with(&src, &[]).verify_headers(&headers),
            Err(Rejection::MissingToken)
        );
        assert_eq!(src.calls(), 0);
    }

    #[test]
    fn header_and_cookie_fallback_are_accepted() {
        let src = kid_a_source();
        let verifier = verifier_with(&src, &[]);
        let mut headers = HeaderMap::new();
        headers.insert(JWT_HEADER, valid_token().parse().unwrap());
        assert!(verifier.verify_headers(&headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            format!("theme=dark; CF_Authorization={}", valid_token()).parse().unwrap(),
        );
        assert!(verifier.verify_headers(&headers).is_ok());
    }

    #[test]
    fn header_wins_over_cookie() {
        let src = kid_a_source();
        let verifier = verifier_with(&src, &[]);
        let cookie = |token: &str| format!("CF_Authorization={token}").parse().unwrap();

        let mut headers = HeaderMap::new();
        headers.insert(JWT_HEADER, valid_token().parse().unwrap());
        headers.insert("cookie", cookie("garbage"));
        assert!(verifier.verify_headers(&headers).is_ok());

        let mut headers = HeaderMap::new();
        headers.insert(JWT_HEADER, "garbage".parse().unwrap());
        headers.insert("cookie", cookie(&valid_token()));
        assert_eq!(verifier.verify_headers(&headers), Err(Rejection::Malformed));
    }

    #[test]
    fn unknown_kid_triggers_rate_limited_refetch() {
        let src = kid_a_source();
        let mut verifier = verifier_with(&src, &[]);
        assert!(verifier.verify(&valid_token()).is_ok());
        assert_eq!(src.calls(), 1);

        src.set(Ok(vec![keys().0.jwk.clone(), keys().1.jwk.clone()]));
        let rotated = sign(&keys().1, "kid-b", claims());
        assert_eq!(verifier.verify(&rotated), Err(Rejection::UnknownKey));
        assert_eq!(src.calls(), 1, "refetch must wait for the interval");

        verifier.refetch_interval = Duration::ZERO;
        assert!(verifier.verify(&rotated).is_ok());
        assert_eq!(src.calls(), 2);
        assert!(verifier.verify(&rotated).is_ok());
        assert_eq!(src.calls(), 2, "a known kid is served from the cache");
    }

    #[test]
    fn empty_key_set_after_good_fetch_keeps_old_keys() {
        let src = kid_a_source();
        let mut verifier = verifier_with(&src, &[]);
        assert!(verifier.verify(&valid_token()).is_ok());

        verifier.refetch_interval = Duration::ZERO;
        src.set(Ok(Vec::new()));
        let rotated = sign(&keys().1, "kid-b", claims());
        assert_eq!(verifier.verify(&rotated), Err(Rejection::UnknownKey));
        assert_eq!(src.calls(), 2);
        assert!(verifier.verify(&valid_token()).is_ok(), "an empty response must not wipe good keys");
    }

    #[test]
    fn invalid_and_non_rsa_keys_are_skipped() {
        let jwk = |kid: &str, kty: &str, n: Option<&str>| Jwk {
            kid: kid.into(),
            kty: kty.into(),
            n: n.map(str::to_string),
            e: Some("AQAB".into()),
        };
        let src = source(Ok(vec![
            jwk("ec", "EC", None),
            jwk("bad-n", "RSA", Some("!!not base64!!")),
            jwk("no-n", "RSA", None),
            keys().0.jwk.clone(),
        ]));
        let verifier = verifier_with(&src, &[]);
        assert!(verifier.verify(&valid_token()).is_ok());
        let bad = sign(&keys().0, "bad-n", claims());
        assert_eq!(verifier.verify(&bad), Err(Rejection::UnknownKey));
    }

    #[test]
    fn email_must_be_allowed_when_list_is_set() {
        let src = kid_a_source();
        assert!(verifier_with(&src, &["dev@example.com"]).verify(&valid_token()).is_ok());
        assert_eq!(
            verifier_with(&src, &["boss@example.com"]).verify(&valid_token()),
            Err(Rejection::EmailNotAllowed)
        );
        let mut c = claims();
        c.as_object_mut().unwrap().remove("email");
        let token = sign(&keys().0, "kid-a", c);
        assert_eq!(
            verifier_with(&src, &["dev@example.com"]).verify(&token),
            Err(Rejection::EmailNotAllowed)
        );
    }

    #[test]
    fn fails_closed_when_keys_cannot_be_fetched_and_retries_sooner() {
        let src = source(Err(FetchError::Connect));
        let mut verifier = verifier_with(&src, &[]);
        assert_eq!(verifier.verify(&valid_token()), Err(Rejection::KeysUnavailable));
        assert_eq!(verifier.verify(&valid_token()), Err(Rejection::KeysUnavailable));
        assert_eq!(src.calls(), 1);

        // An empty cache uses the short retry interval, not the 60s refetch interval.
        verifier.empty_retry_interval = Duration::ZERO;
        src.set(Ok(vec![keys().0.jwk.clone()]));
        assert!(verifier.verify(&valid_token()).is_ok());
        assert_eq!(src.calls(), 2);

        let empty = source(Ok(Vec::new()));
        verifier = verifier_with(&empty, &[]);
        assert_eq!(verifier.verify(&valid_token()), Err(Rejection::KeysUnavailable));
    }

    /// Serves `kid-a` immediately on the first call and parks every later call until released.
    struct SlowSource {
        calls: AtomicUsize,
        entered: Barrier,
        release: Barrier,
    }

    impl KeySource for Arc<SlowSource> {
        fn fetch(&self) -> Result<Vec<Jwk>, FetchError> {
            if self.calls.fetch_add(1, Ordering::SeqCst) > 0 {
                self.entered.wait();
                self.release.wait();
            }
            Ok(vec![keys().0.jwk.clone()])
        }
    }

    #[test]
    fn cache_hits_do_not_wait_on_an_in_flight_fetch() {
        let src = Arc::new(SlowSource {
            calls: AtomicUsize::new(0),
            entered: Barrier::new(2),
            release: Barrier::new(2),
        });
        let config = AccessConfig::new(TEAM, AUD, &[]).unwrap();
        let mut verifier = Verifier::new(config, Box::new(src.clone()));
        verifier.refetch_interval = Duration::ZERO;
        let verifier = Arc::new(verifier);
        assert!(verifier.verify(&valid_token()).is_ok());

        let rotated = sign(&keys().1, "kid-b", claims());
        let background = {
            let verifier = verifier.clone();
            let rotated = rotated.clone();
            std::thread::spawn(move || verifier.verify(&rotated))
        };
        src.entered.wait();
        assert!(verifier.verify(&valid_token()).is_ok(), "a cache hit must not block");
        assert_eq!(verifier.verify(&rotated), Err(Rejection::UnknownKey));
        assert_eq!(src.calls.load(Ordering::SeqCst), 2, "only one fetch may be in flight");
        src.release.wait();
        assert_eq!(background.join().unwrap(), Err(Rejection::UnknownKey));
    }

    #[test]
    fn rejection_logging_is_rate_limited_per_kind() {
        let src = kid_a_source();
        let verifier = verifier_with(&src, &[]);
        let t0 = Instant::now();
        assert!(verifier.should_log(Rejection::Expired, t0));
        assert!(!verifier.should_log(Rejection::Expired, t0 + Duration::from_secs(1)));
        assert!(verifier.should_log(Rejection::InvalidAudience, t0));
        assert!(verifier.should_log(Rejection::Expired, t0 + REJECTION_LOG_INTERVAL));
    }

    #[test]
    fn header_presence_alone_is_not_enough() {
        let src = kid_a_source();
        let mut headers = HeaderMap::new();
        headers.insert(JWT_HEADER, "not-a-jwt".parse().unwrap());
        assert_eq!(
            verifier_with(&src, &[]).verify_headers(&headers),
            Err(Rejection::Malformed)
        );
    }

    /// Echoes the identity the gate attached, so a 200 proves the request reached the inner handler.
    async fn echo_identity(identity: Option<Extension<AccessIdentity>>) -> String {
        identity.map(|Extension(AccessIdentity(id))| id).unwrap_or_default()
    }

    fn gated_router(verifier: Option<Arc<Verifier>>) -> Router {
        Router::new()
            .route("/", get(echo_identity))
            .route("/api/login", post(echo_identity))
            .route("/ws", get(echo_identity))
            .layer(axum::middleware::from_fn_with_state(verifier, gate))
    }

    async fn call(router: &Router, method: &str, path: &str, token: Option<&str>) -> (StatusCode, String) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header(JWT_HEADER, token);
        }
        let response = router
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn gate_rejects_every_route_without_a_token_and_admits_a_valid_one() {
        let src = kid_a_source();
        let router = gated_router(Some(Arc::new(verifier_with(&src, &[]))));
        for (method, path) in [("GET", "/"), ("POST", "/api/login"), ("GET", "/ws"), ("GET", "/unknown")] {
            let (status, _) = call(&router, method, path, None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {path}");
        }
        let token = valid_token();
        for (method, path) in [("GET", "/"), ("POST", "/api/login"), ("GET", "/ws")] {
            let (status, body) = call(&router, method, path, Some(&token)).await;
            assert_eq!(status, StatusCode::OK, "{method} {path}");
            assert_eq!(body, "dev@example.com", "the verified identity reaches the handler");
        }
    }

    #[tokio::test]
    async fn gate_without_verifier_passes_through() {
        let router = gated_router(None);
        assert_eq!(call(&router, "GET", "/", None).await, (StatusCode::OK, String::new()));
    }

    #[test]
    fn login_limit_key_prefers_verified_identity() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(login_limit_key(ip, None), LimitKey::Ip(ip));
        assert_eq!(
            login_limit_key(ip, Some(&AccessIdentity("dev@example.com".into()))),
            LimitKey::Identity("dev@example.com".into())
        );
    }

    #[test]
    fn team_domain_is_normalized_and_validated() {
        assert_eq!(normalize_team_domain(" https://MyTeam.cloudflareaccess.com/ ").unwrap(), TEAM);
        assert_eq!(normalize_team_domain(TEAM).unwrap(), TEAM);
        for bad in [
            "",
            "cloudflareaccess.com",
            ".cloudflareaccess.com",
            "evil.com",
            "myteam.cloudflareaccess.com.evil.com",
            "myteam.cloudflareaccess.com/path",
            "user@myteam.cloudflareaccess.com",
            "myteam.cloudflareaccess.com:8443",
        ] {
            assert!(normalize_team_domain(bad).is_err(), "{bad} must be rejected");
        }
        assert!(AccessConfig::new(TEAM, "  ", &[]).is_err());
        assert_eq!(
            normalize_emails(&[" A@x.com ".into(), "a@x.com".into(), "".into()]),
            vec!["a@x.com".to_string()]
        );
    }
}

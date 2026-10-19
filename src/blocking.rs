//! Blocking ORCID **Public API** client (feature `blocking`, over `ureq` 3,
//! no async runtime) for **occasional single-record retrieval**. It reads
//! only ORCID's Public API (`https://pub.orcid.org/v3.0`), anonymously or
//! with an optional 2-legged `/read-public` token, and writes nothing.
//!
//! yorc-rs is an independent open-source project. It is not affiliated with,
//! endorsed by or sponsored by ORCID, Inc. It does not provide access to the
//! ORCID Sandbox or the ORCID Member API: this client reads only ORCID's
//! Public API (`pub.orcid.org`).
//!
//! For anything more than a few records, use ORCID's Public Data File (the
//! yearly CC0 dump) and this crate's `dump` reader instead: it works on
//! local files and has no rate limit. This client has no crawling, paging
//! through everyone or parallel-fetching helpers, and none will be added.
//!
//! # Hosts
//!
//! A client is built for one [`Access`], fixed at construction
//! ([`Client::new`] / [`Client::with_limiter`]; [`Client::config`] is
//! read-only). Each is a closed set of base URLs:
//!
//! | access | API base | OAuth base | token realm |
//! |---|---|---|---|
//! | [`Access::Public`] | `https://pub.orcid.org/v3.0` | `https://orcid.org/oauth` | [`Realm::Production`] |
//! | [`Access::Custom`] (tests, mock servers) | `api` as given | `oauth` as given | [`Realm::Custom`] |
//!
//! The OAuth base serves one call only: the optional `/read-public` token
//! ([`Client::read_public_token`]). Reads need no token.
//!
//! # Enforcement
//!
//! * Every request URL, the token call included, is checked against the
//!   base URLs ([`Access::permits`]) just before it is sent; a URL outside
//!   them is refused with [`ClientError::Usage`] and nothing is sent. A
//!   `Custom` URL on an ORCID host is refused too: ORCID is reached only
//!   through `Public`, so only at `pub.orcid.org` and
//!   `orcid.org/oauth/token`.
//! * Tokens carry the [`Realm`] they were issued for ([`Token::realm`]); an
//!   empty token, a token of another realm and a token without the
//!   `/read-public` scope are refused before sending
//!   ([`Access::check_token`]).
//! * Redirects are never followed.
//!
//! # Pacing: one request every 45 s to ORCID
//!
//! Every request of a `Public` client (reads, search, the token call and
//! retries) goes through one process-wide limiter ([`RateLimiter::orcid`]):
//! at most one request every [`ORCID_INTERVAL`] (45 s), strictly one at a
//! time, across every client, clone and thread of the process. A limiter
//! passed to [`Client::with_limiter`] can only slow a client further: a
//! faster rate is clamped to the cap, silently (the effective spacing is
//! [`Client::min_interval`]). Only `Custom` (mock servers) may go faster,
//! and not when its base URL is an ORCID host ([`Access::is_orcid`]).
//!
//! The cap is for ORCID's own hosts (`orcid.org` and its subdomains). It
//! does not apply to Public Data File downloads (Figshare or ORCID's S3/CDN
//! mirrors) or to the `dump` reader, which works on local files and is
//! unthrottled.
//!
//! Retries: GET is retried on 5xx and transport errors, at most
//! [`ORCID_MAX_ATTEMPTS`] (3) attempts in all for ORCID hosts, each paced
//! by the limiter, waiting `Retry-After` when it is longer. POST (the token
//! call) is never retried: a 5xx cannot tell whether the call was applied.
//! A 429 is never retried: the client stops, and the shared limiter is
//! paused for `Retry-After` (at least the 45 s interval), so no request
//! reaches ORCID before it has passed.
//!
//! Non-2xx answers become [`ClientError::Api`] with ORCID's `error-3.0` body
//! parsed into [`OrcidError`].
//!
//! ```no_run
//! use yorc::blocking::{Access, Client, Config};
//!
//! let client = Client::new(Config::new(Access::Public));
//! let record = client.record("0000-0002-1825-0097", None)?;
//! println!("{:?}", record.person.and_then(|p| p.name));
//! # Ok::<(), yorc::blocking::ClientError>(())
//! ```

use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde_json::Value;

use crate::id::OrcidId;
use crate::model::*;
use crate::xml;

/// The ORCID XML media type.
pub const ORCID_XML: &str = "application/vnd.orcid+xml";
/// The ORCID JSON media type.
pub const ORCID_JSON: &str = "application/vnd.orcid+json";
/// ORCID's limit on put-codes per bulk read ([`Client::works_bulk`]).
pub const MAX_BULK: usize = 100;
/// ORCID's limit on `rows` per search page ([`Client::search`]).
pub const MAX_SEARCH_ROWS: u32 = 1000;
/// The scope of the optional client-credentials token
/// ([`Client::read_public_token`]), the only scope this client sends.
pub const SCOPE_READ_PUBLIC: &str = "/read-public";

/// ORCID's Public API, the only API this client reads.
pub const PUBLIC_API: &str = "https://pub.orcid.org/v3.0";
/// ORCID's OAuth base, for the `/read-public` token call only.
pub const OAUTH: &str = "https://orcid.org/oauth";

/// The spacing of requests to ORCID's hosts (every access but `Custom`):
/// one request every 45 s, the maximum rate. A cap: slower is allowed,
/// faster is clamped.
pub const ORCID_INTERVAL: Duration = Duration::from_secs(45);
/// Attempts per request to ORCID's hosts, retries included.
pub const ORCID_MAX_ATTEMPTS: u32 = 3;
const USER_AGENT: &str = concat!("yorc-rs/", env!("CARGO_PKG_VERSION"));

// -------------------------------------------------------------------- access

/// Where a token was issued, so which clients may send it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Realm {
    /// `orcid.org` (the `Public` access).
    Production,
    /// A `Custom` client (tests).
    Custom,
}

/// What a URL is used for ([`Access::permits`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Purpose {
    /// A GET of the API.
    Read,
    /// The OAuth token endpoint.
    OAuth,
}

/// The access of a client: which hosts it talks to. See the module
/// documentation for the host table.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Access {
    /// ORCID's Public API (`pub.orcid.org`): public reads only.
    Public,
    /// Explicit base URLs, for mock servers in tests: the same reads, every
    /// request to `api` (or `oauth`). Not paced by the ORCID limiter; a URL
    /// on an ORCID host is refused ([`Access::permits`]).
    Custom {
        /// API base, ending in `/v3.0`.
        api: String,
        /// OAuth base, ending in `/oauth`.
        oauth: String,
    },
}

fn trim(s: &str) -> String {
    s.trim_end_matches('/').to_string()
}

/// `url` is `base` or below it (`base/...`, `base?...`).
fn under(url: &str, base: &str) -> bool {
    !base.is_empty()
        && url
            .strip_prefix(base)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('?'))
}

/// The host of `url` is `orcid.org` or a subdomain of it.
fn is_orcid_host(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or_default();
    let host = host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    host == "orcid.org" || host.ends_with(".orcid.org")
}

impl Access {
    /// The realm of the tokens this client issues and accepts.
    pub fn realm(&self) -> Realm {
        match self {
            Access::Public => Realm::Production,
            Access::Custom { .. } => Realm::Custom,
        }
    }

    /// Whether requests go to ORCID's hosts, and so through the 45 s
    /// limiter: `Public`, and a `Custom` whose `api` or `oauth` host is
    /// `orcid.org` or one of its subdomains (so `Custom` cannot be used to
    /// go faster against ORCID).
    pub fn is_orcid(&self) -> bool {
        match self {
            Access::Public => true,
            Access::Custom { api, oauth } => is_orcid_host(api) || is_orcid_host(oauth),
        }
    }

    /// The API base, no trailing slash.
    pub fn api_base(&self) -> String {
        match self {
            Access::Public => PUBLIC_API.into(),
            Access::Custom { api, .. } => trim(api),
        }
    }

    /// The OAuth base (`/token`), no trailing slash.
    pub fn oauth_base(&self) -> String {
        match self {
            Access::Public => OAUTH.into(),
            Access::Custom { oauth, .. } => trim(oauth),
        }
    }

    /// The host allowlist: whether `url` is inside the base URL for
    /// `purpose` (reads: the API base; OAuth: the OAuth base). The URL must
    /// start with the base followed by `/`, `?` or nothing, so the scheme,
    /// host and port are exactly the base's
    /// (`https://pub.orcid.org.evil.example/` and
    /// `https://pub.orcid.org@evil.example/` are refused); whitespace,
    /// control characters and `\` are refused anywhere. A `Custom` access
    /// refuses every URL on an ORCID host.
    ///
    /// ```
    /// use yorc::blocking::{Access, Purpose};
    ///
    /// let p = Access::Public;
    /// assert!(p.permits(Purpose::Read, "https://pub.orcid.org/v3.0/0000-0002-1825-0097/record"));
    /// assert!(p.permits(Purpose::OAuth, "https://orcid.org/oauth/token"));
    /// assert!(!p.permits(Purpose::Read, "https://orcid.org/oauth/token"));
    /// assert!(!p.permits(Purpose::Read, "https://pub.orcid.org.evil.example/v3.0/x"));
    /// ```
    pub fn permits(&self, purpose: Purpose, url: &str) -> bool {
        if url.bytes().any(|b| b <= b' ' || b == 0x7f || b == b'\\') {
            return false;
        }
        if matches!(self, Access::Custom { .. }) && is_orcid_host(url) {
            return false;
        }
        let base = match purpose {
            Purpose::Read => self.api_base(),
            Purpose::OAuth => self.oauth_base(),
        };
        under(url, &base)
    }

    /// Refuses an empty token, a token of another realm and a token without
    /// the `/read-public` scope.
    ///
    /// ```
    /// use yorc::blocking::{Access, Realm, Token};
    ///
    /// let t = Token::new(Realm::Production, "5f1c...", "/read-public");
    /// assert!(Access::Public.check_token(&t).is_ok());
    /// let other = Token::new(Realm::Production, "5f1c...", "/authenticate");
    /// assert!(Access::Public.check_token(&other).is_err());
    /// ```
    pub fn check_token(&self, token: &Token) -> ClientResult<()> {
        if token.access_token.trim().is_empty() {
            return Err(ClientError::Usage("empty access token".into()));
        }
        if token.realm != self.realm() {
            return Err(ClientError::Usage(format!(
                "a {:?} token cannot be used by a {} client (realm {:?}); nothing was sent",
                token.realm,
                self.name(),
                self.realm()
            )));
        }
        if !token.has_scope(SCOPE_READ_PUBLIC) {
            return Err(ClientError::Usage(format!(
                "this client sends only {SCOPE_READ_PUBLIC} tokens, not {:?}; nothing was sent",
                token.scope
            )));
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        match self {
            Access::Public => "Public",
            Access::Custom { .. } => "Custom",
        }
    }
}

// --------------------------------------------------------------------- token

/// An OAuth access token (a 2-legged `/read-public` token), with the realm
/// it was issued for.
#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    /// The bearer token (`Debug` prints `<redacted>`).
    pub access_token: String,
    /// The refresh token, if ORCID issued one (`Debug` prints `<redacted>`).
    pub refresh_token: Option<String>,
    /// Granted scopes, space-separated (`/read-public`).
    pub scope: String,
    /// When the token was received.
    pub granted_at: SystemTime,
    /// `granted_at` + `expires_in`, if ORCID sent `expires_in`.
    pub expires_at: Option<SystemTime>,
    /// The realm it was issued for: only clients of that realm send it.
    pub realm: Realm,
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Token")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("scope", &self.scope)
            .field("granted_at", &self.granted_at)
            .field("expires_at", &self.expires_at)
            .field("realm", &self.realm)
            .finish()
    }
}

impl Token {
    /// A token obtained elsewhere (e.g. from an environment variable), for
    /// `realm`, with the scopes it was granted. An empty `access_token` reads
    /// anonymously.
    ///
    /// ```
    /// use yorc::blocking::{Realm, Token};
    ///
    /// let t = Token::new(Realm::Production, "5f1c...", "/read-public");
    /// assert!(t.has_scope("/read-public"));
    /// assert!(!format!("{t:?}").contains("5f1c")); // Debug redacts the token
    /// ```
    pub fn new(realm: Realm, access_token: impl Into<String>, scope: impl Into<String>) -> Token {
        Token {
            access_token: access_token.into(),
            refresh_token: None,
            scope: scope.into(),
            granted_at: SystemTime::now(),
            expires_at: None,
            realm,
        }
    }

    /// Whether the token has expired at `now`.
    pub fn is_expired(&self, now: SystemTime) -> bool {
        self.expires_at.is_some_and(|t| now >= t)
    }

    /// Whether `scope` is among the granted scopes.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scope.split_whitespace().any(|s| s == scope)
    }
}

/// Percent-encodes a query or form value (RFC 3986 unreserved kept).
///
/// ```
/// assert_eq!(yorc::blocking::encode_component("a b/c~"), "a%20b%2Fc~");
/// ```
pub fn encode_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

// -------------------------------------------------------------------- client

/// Client settings. `Config::new` gives the defaults.
///
/// ```
/// use std::time::Duration;
/// use yorc::blocking::{Access, Config};
///
/// let mut config = Config::new(Access::Public);
/// config.timeout = Duration::from_secs(20);
/// assert_eq!(config.access.api_base(), "https://pub.orcid.org/v3.0");
/// ```
#[derive(Clone, Debug)]
pub struct Config {
    /// The access (fixed once the client is built).
    pub access: Access,
    /// Retries after the first attempt (GET only; for
    /// ORCID's hosts at most [`ORCID_MAX_ATTEMPTS`] attempts in all).
    pub max_retries: u32,
    /// Upper bound for one `Retry-After` wait.
    pub max_retry_after: Duration,
    /// First exponential-backoff wait (doubles per attempt, capped at 30 s).
    pub backoff_base: Duration,
    /// Whole-request timeout.
    pub timeout: Duration,
    /// Largest response body accepted.
    pub max_body: u64,
}

impl Config {
    /// The defaults: 2 retries, `Retry-After` honoured up to an
    /// hour, 500 ms backoff base (requests to ORCID are 45 s apart anyway),
    /// 60 s timeout, 256 MiB body limit.
    pub fn new(access: Access) -> Config {
        Config {
            access,
            max_retries: ORCID_MAX_ATTEMPTS - 1,
            max_retry_after: Duration::from_secs(3600),
            backoff_base: Duration::from_millis(500),
            timeout: Duration::from_secs(60),
            max_body: 256 * 1024 * 1024,
        }
    }
}

/// Request pacing shared by every client and thread holding the same `Arc`:
/// one request per interval, no bursts, and (for the ORCID limiter) one
/// request at a time.
///
/// Clients of ORCID's hosts always go through [`RateLimiter::orcid`]; a
/// limiter of their own can only make them slower. `Custom` clients (mock
/// servers) use only their own.
///
/// ```
/// use std::sync::Arc;
/// use std::time::Duration;
/// use yorc::blocking::{Access, Client, Config, RateLimiter};
///
/// // two clients sharing the process-wide ORCID limiter (one request per 45 s);
/// // the second is slower still: one request per minute
/// let a = Client::new(Config::new(Access::Public));
/// let slow = Arc::new(RateLimiter::with_interval(Duration::from_secs(60)));
/// let b = Client::with_limiter(Config::new(Access::Public), slow);
/// # let _ = (a, b);
/// ```
#[derive(Debug)]
pub struct RateLimiter {
    interval: Duration,
    next: Mutex<Instant>,
    /// Held across a request to ORCID: strictly one at a time.
    serial: Mutex<()>,
}

impl RateLimiter {
    /// `rate` requests per second; non-positive or non-finite → one per
    /// [`ORCID_INTERVAL`].
    pub fn new(rate: f64) -> RateLimiter {
        if rate.is_finite() && rate > 0.0 {
            RateLimiter::with_interval(Duration::from_secs_f64(1.0 / rate))
        } else {
            RateLimiter::default()
        }
    }

    /// One request per `interval`.
    pub fn with_interval(interval: Duration) -> RateLimiter {
        RateLimiter {
            interval,
            next: Mutex::new(Instant::now()),
            serial: Mutex::new(()),
        }
    }

    /// The process-wide limiter of ORCID's hosts: one request every
    /// [`ORCID_INTERVAL`], one at a time, shared by every client.
    pub fn orcid() -> Arc<RateLimiter> {
        static ORCID: OnceLock<Arc<RateLimiter>> = OnceLock::new();
        Arc::clone(ORCID.get_or_init(|| Arc::new(RateLimiter::default())))
    }

    /// The spacing between two requests.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Reserves the next slot and sleeps until it arrives.
    pub fn acquire(&self) {
        self.acquire_spaced(Duration::ZERO);
    }

    /// Reserves the next slot, at least `min` after the one before, and
    /// sleeps until it arrives.
    fn acquire_spaced(&self, min: Duration) {
        let wait = {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let slot = if *next > now { *next } else { now };
            *next = slot + self.interval.max(min);
            slot.saturating_duration_since(now)
        };
        if !wait.is_zero() {
            thread::sleep(wait);
        }
    }

    /// No slot before `now + d`.
    pub fn pause(&self, d: Duration) {
        let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
        let until = Instant::now() + d;
        if until > *next {
            *next = until;
        }
    }
}

impl Default for RateLimiter {
    /// One request per [`ORCID_INTERVAL`] (45 s).
    fn default() -> RateLimiter {
        RateLimiter::with_interval(ORCID_INTERVAL)
    }
}

/// Client errors.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// ORCID's OAuth token endpoint refused (e.g. `invalid_client`).
    OAuth {
        /// ORCID's `error` (e.g. `invalid_client`).
        error: String,
        /// ORCID's `error_description`.
        description: Option<String>,
    },
    /// Connection, TLS, timeout or body-read failure (after retries).
    Transport(String),
    /// A non-2xx answer (after retries); `error` is the `error-3.0` body.
    Api {
        /// HTTP status.
        status: u16,
        /// The `error-3.0` body, if the response was one.
        error: Option<OrcidError>,
        /// The response body (lossily decoded).
        body: Box<str>,
    },
    /// The response is not the document expected.
    Decode(String),
    /// Bad arguments, or a URL or token the access does not allow; nothing
    /// was sent.
    Usage(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::OAuth { error, description } => {
                write!(f, "OAuth error {error}")?;
                if let Some(d) = description {
                    write!(f, ": {d}")?;
                }
                Ok(())
            }
            ClientError::Transport(m) => write!(f, "transport error: {m}"),
            ClientError::Api { status, error, .. } => {
                write!(f, "ORCID API answered {status}")?;
                write_orcid_error(f, error.as_ref())
            }
            ClientError::Decode(m) => write!(f, "unexpected response: {m}"),
            ClientError::Usage(m) => f.write_str(m),
        }
    }
}

fn write_orcid_error(f: &mut fmt::Formatter<'_>, e: Option<&OrcidError>) -> fmt::Result {
    if let Some(e) = e {
        if let Some(code) = e.error_code {
            write!(f, " (error {code})")?;
        }
        if let Some(m) = e.developer_message.as_ref().or(e.user_message.as_ref()) {
            write!(f, ": {m}")?;
        }
    }
    Ok(())
}

impl std::error::Error for ClientError {}

/// `Result` with a [`ClientError`].
pub type ClientResult<T> = std::result::Result<T, ClientError>;

struct Answer {
    status: u16,
    /// The body exactly as received (at most `Config::max_body` bytes).
    bytes: Vec<u8>,
    /// The `Retry-After` header.
    retry_after: Option<String>,
}

impl Answer {
    fn text(&self) -> ClientResult<&str> {
        std::str::from_utf8(&self.bytes)
            .map_err(|e| ClientError::Transport(format!("reading the response body: {e}")))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Method {
    Get,
    Post,
}

struct Request<'a> {
    method: Method,
    purpose: Purpose,
    url: &'a str,
    accept: &'a str,
    content_type: Option<&'a str>,
    bearer: Option<&'a str>,
    body: Option<&'a [u8]>,
}

#[cfg(feature = "json")]
fn json_doc(kind: &str, text: &str) -> Option<Document> {
    crate::json::from_str(kind, text).ok()
}

#[cfg(not(feature = "json"))]
fn json_doc(_: &str, _: &str) -> Option<Document> {
    None
}

/// The ORCID API client. Cheap to clone; clones share agent and limiters.
///
/// ```no_run
/// use yorc::blocking::{Access, Client, Config};
///
/// let public = Client::new(Config::new(Access::Public));
/// let works = public.works("0000-0002-1825-0097", None)?;
/// println!("{} groups", works.groups.len());
/// # Ok::<(), yorc::blocking::ClientError>(())
/// ```
#[derive(Clone)]
pub struct Client {
    agent: ureq::Agent,
    config: Config,
    limiter: Arc<RateLimiter>,
    /// The process-wide ORCID limiter, for ORCID's hosts.
    orcid: Option<Arc<RateLimiter>>,
}

fn retry_after(value: Option<&str>, cap: Duration) -> Option<Duration> {
    let secs: u64 = value?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(cap))
}

/// The iD of a call, in any accepted form; anything else is a usage error.
fn parse_id(orcid: &str) -> ClientResult<OrcidId> {
    OrcidId::parse(orcid).map_err(|e| ClientError::Usage(e.to_string()))
}

impl Client {
    /// A client. For ORCID's hosts it uses the process-wide limiter
    /// ([`RateLimiter::orcid`]); a `Custom` client gets its own limiter at
    /// the default interval (45 s; use [`Client::with_limiter`] in tests).
    pub fn new(config: Config) -> Client {
        let limiter = if config.access.is_orcid() {
            RateLimiter::orcid()
        } else {
            Arc::new(RateLimiter::default())
        };
        Client::with_limiter(config, limiter)
    }

    /// A client sharing `limiter` with other clients or threads. For ORCID's
    /// hosts the process-wide limiter applies as well, so `limiter` can only
    /// make the client slower (a faster rate is clamped to the 45 s cap).
    pub fn with_limiter(config: Config, limiter: Arc<RateLimiter>) -> Client {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(config.timeout))
            .user_agent(USER_AGENT)
            // never follow a redirect: a deprecated record answers 301 with an
            // error document, and following it would read another iD's record
            .max_redirects(0)
            .build()
            .into();
        let orcid = config.access.is_orcid().then(RateLimiter::orcid);
        Client {
            agent,
            config,
            limiter,
            orcid,
        }
    }

    /// The settings this client was built with (read-only: the access cannot
    /// change).
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The access.
    pub fn access(&self) -> &Access {
        &self.config.access
    }

    /// The effective spacing of this client's requests: its limiter's
    /// interval, at least [`ORCID_INTERVAL`] for ORCID's hosts.
    pub fn min_interval(&self) -> Duration {
        if self.orcid.is_some() {
            self.limiter.interval().max(ORCID_INTERVAL)
        } else {
            self.limiter.interval()
        }
    }

    fn max_attempts(&self) -> u32 {
        let wanted = self.config.max_retries.saturating_add(1);
        if self.orcid.is_some() {
            wanted.min(ORCID_MAX_ATTEMPTS)
        } else {
            wanted
        }
    }

    fn backoff(&self, attempt: u32) -> Duration {
        let base = self.config.backoff_base.as_millis() as u64;
        Duration::from_millis(base.saturating_mul(1u64 << attempt.min(16)).min(30_000))
    }

    /// One request, sent as given.
    fn send_once(&self, req: &Request<'_>) -> Result<Answer, String> {
        let result = match req.method {
            Method::Get => {
                let mut rb = self.agent.get(req.url).header("Accept", req.accept);
                if let Some(t) = req.bearer {
                    rb = rb.header("Authorization", format!("Bearer {t}").as_str());
                }
                rb.call()
            }
            Method::Post => {
                let mut rb = self.agent.post(req.url).header("Accept", req.accept);
                if let Some(ct) = req.content_type {
                    rb = rb.header("Content-Type", ct);
                }
                if let Some(t) = req.bearer {
                    rb = rb.header("Authorization", format!("Bearer {t}").as_str());
                }
                rb.send(req.body.unwrap_or(&[]))
            }
        };
        let mut resp = result.map_err(|e| e.to_string())?;
        let header = |name: &str| {
            resp.headers()
                .get(name)
                .and_then(|v| v.to_str().ok())
                .map(str::to_string)
        };
        let retry_after = header("retry-after");
        let status = resp.status().as_u16();
        let bytes = resp
            .body_mut()
            .with_config()
            .limit(self.config.max_body)
            .read_to_vec()
            .map_err(|e| format!("reading the response body: {e}"))?;
        Ok(Answer {
            status,
            bytes,
            retry_after,
        })
    }

    /// One request, with pacing and retries:
    ///
    /// * the URL must be inside the access's base URLs for its purpose
    ///   ([`Access::permits`]), checked before every attempt;
    /// * ORCID's hosts: the process-wide limiter (45 s, one at a time) and
    ///   at most [`ORCID_MAX_ATTEMPTS`] attempts;
    /// * GET is retried on 5xx and transport errors, never POST; a 429 is
    ///   never retried: the limiters are paused for its `Retry-After` (at
    ///   least the interval) and the 429 is returned.
    fn exchange(&self, req: &Request<'_>) -> ClientResult<Answer> {
        let idempotent = req.method != Method::Post;
        let attempts = self.max_attempts();
        let mut attempt = 0;
        loop {
            if !self.config.access.permits(req.purpose, req.url) {
                return Err(ClientError::Usage(format!(
                    "refused: {} is outside the hosts of this client's access; nothing was sent",
                    req.url
                )));
            }
            let sent = {
                let _one_at_a_time = self
                    .orcid
                    .as_ref()
                    .map(|o| o.serial.lock().unwrap_or_else(|e| e.into_inner()));
                if let Some(o) = &self.orcid {
                    o.acquire_spaced(ORCID_INTERVAL);
                    if !Arc::ptr_eq(o, &self.limiter) {
                        self.limiter.acquire();
                    }
                } else {
                    self.limiter.acquire();
                }
                self.send_once(req)
            };
            attempt += 1;
            let last = attempt >= attempts;
            match sent {
                Ok(ans) => {
                    let ra = retry_after(ans.retry_after.as_deref(), self.config.max_retry_after);
                    if ans.status == 429 {
                        // stop; nothing reaches the host before Retry-After
                        // (and never sooner than the 45 s spacing for ORCID)
                        if let Some(d) = ra {
                            self.pause(d);
                        }
                        return Ok(ans);
                    }
                    let retryable = idempotent && (500..=599).contains(&ans.status);
                    if !retryable || last {
                        return Ok(ans);
                    }
                    match ra {
                        Some(d) => self.pause(d),
                        None => thread::sleep(self.backoff(attempt - 1)),
                    }
                }
                Err(e) => {
                    if !idempotent || last {
                        return Err(ClientError::Transport(e));
                    }
                    thread::sleep(self.backoff(attempt - 1));
                }
            }
        }
    }

    fn pause(&self, d: Duration) {
        if let Some(o) = &self.orcid {
            o.pause(d);
        }
        self.limiter.pause(d);
    }

    // ------------------------------------------------------------- OAuth

    /// The optional 2-legged `/read-public` token for an ORCID API client
    /// (`POST {oauth}/token`, `grant_type=client_credentials`; never
    /// retried), with this client's realm. Reads need no token: this is for
    /// callers who want their requests identified by their client id. It
    /// names no user; there is no sign-in (3-legged) OAuth here.
    pub fn read_public_token(&self, client_id: &str, client_secret: &str) -> ClientResult<Token> {
        let form = format!(
            "client_id={}&client_secret={}&grant_type=client_credentials&scope={}",
            encode_component(client_id),
            encode_component(client_secret),
            encode_component(SCOPE_READ_PUBLIC)
        );
        let url = format!("{}/token", self.config.access.oauth_base());
        let ans = self.exchange(&Request {
            method: Method::Post,
            purpose: Purpose::OAuth,
            url: &url,
            accept: "application/json",
            content_type: Some("application/x-www-form-urlencoded"),
            bearer: None,
            body: Some(form.as_bytes()),
        })?;
        let body = ans.text()?;
        let json: Value = serde_json::from_str(body).map_err(|e| {
            if (200..=299).contains(&ans.status) {
                ClientError::Decode(format!("token response is not JSON: {e}"))
            } else {
                ClientError::Api {
                    status: ans.status,
                    error: None,
                    body: body.into(),
                }
            }
        })?;
        let text = |k: &str| json.get(k).and_then(Value::as_str).map(str::to_string);
        if !(200..=299).contains(&ans.status) {
            return Err(ClientError::OAuth {
                error: text("error").unwrap_or_else(|| format!("HTTP {}", ans.status)),
                description: text("error_description"),
            });
        }
        token_from_response(&json, self.config.access.realm(), SystemTime::now())
    }

    // ------------------------------------------------------------- reads

    fn read(&self, url_rest: &str, token: Option<&Token>) -> ClientResult<Answer> {
        // an empty token reads anonymously
        let token = token.filter(|t| !t.access_token.is_empty());
        if let Some(t) = token {
            self.config.access.check_token(t)?;
        }
        let url = format!("{}{url_rest}", self.config.access.api_base());
        let bearer = token.map(|t| t.access_token.as_str());
        self.exchange(&Request {
            method: Method::Get,
            purpose: Purpose::Read,
            url: &url,
            accept: ORCID_XML,
            content_type: None,
            bearer,
            body: None,
        })
    }

    fn get_doc(&self, orcid: &str, token: Option<&Token>, rest: &str) -> ClientResult<Document> {
        let id = parse_id(orcid)?;
        self.get_url(&format!("/{id}{rest}"), token)
    }

    fn get_url(&self, rest: &str, token: Option<&Token>) -> ClientResult<Document> {
        let ans = self.read(rest, token)?;
        if !(200..=299).contains(&ans.status) {
            return Err(api_error(&ans));
        }
        xml::from_str(ans.text()?).map_err(|e| ClientError::Decode(e.to_string()))
    }

    /// `GET /{orcid}/record`, the response body exactly as received (so a
    /// caller can hash and store the exact bytes). Returns `(status, body)`
    /// for 2xx and for ORCID's error documents of a deprecated (301) or
    /// locked (409) record, which are answers about the record, not
    /// failures; any other status is [`ClientError::Api`]. Redirects are
    /// never followed.
    ///
    /// ```no_run
    /// use yorc::blocking::{Access, Client, Config};
    ///
    /// let client = Client::new(Config::new(Access::Public));
    /// let (status, bytes) = client.record_raw("0000-0002-1825-0097", None)?;
    /// if status == 200 {
    ///     std::fs::write("0000-0002-1825-0097.xml", &bytes).expect("write");
    /// }
    /// # Ok::<(), yorc::blocking::ClientError>(())
    /// ```
    pub fn record_raw(&self, orcid: &str, token: Option<&Token>) -> ClientResult<(u16, Vec<u8>)> {
        let id = parse_id(orcid)?;
        let ans = self.read(&format!("/{id}/record"), token)?;
        match ans.status {
            200..=299 | 301 | 409 => Ok((ans.status, ans.bytes)),
            _ => Err(api_error(&ans)),
        }
    }

    /// `GET /{orcid}/{section}` (e.g. `"educations"`, `"fundings"`,
    /// `"keywords"`).
    pub fn section(
        &self,
        orcid: &str,
        token: Option<&Token>,
        section: &str,
    ) -> ClientResult<Document> {
        let ok = !section.is_empty()
            && section
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-/,".contains(&b))
            && !section.contains("..");
        if !ok {
            return Err(ClientError::Usage(format!(
                "not a section path: {section:?}"
            )));
        }
        self.get_doc(orcid, token, &format!("/{section}"))
    }

    /// `GET /{orcid}/record`.
    pub fn record(&self, orcid: &str, token: Option<&Token>) -> ClientResult<Record> {
        match self.get_doc(orcid, token, "/record")?.root {
            Root::Record(r) => Ok(r),
            other => Err(unexpected("record", &other)),
        }
    }

    /// `GET /{orcid}/person`.
    pub fn person(&self, orcid: &str, token: Option<&Token>) -> ClientResult<Person> {
        match self.get_doc(orcid, token, "/person")?.root {
            Root::Person(p) => Ok(p),
            other => Err(unexpected("person", &other)),
        }
    }

    /// `GET /{orcid}/works` (grouped summaries).
    pub fn works(&self, orcid: &str, token: Option<&Token>) -> ClientResult<Works> {
        match self.get_doc(orcid, token, "/works")?.root {
            Root::Works(w) => Ok(w),
            other => Err(unexpected("works", &other)),
        }
    }

    /// `GET /{orcid}/work/{put_code}` (full work).
    pub fn work(
        &self,
        orcid: &str,
        token: Option<&Token>,
        put_code: PutCode,
    ) -> ClientResult<Work> {
        match self
            .get_doc(orcid, token, &format!("/work/{put_code}"))?
            .root
        {
            Root::Work(w) => Ok(w),
            other => Err(unexpected("work", &other)),
        }
    }

    /// `GET /{orcid}/works/{pc,pc,...}`: 1..=100 full works; missing ones come
    /// back as [`BulkItem::Error`].
    pub fn works_bulk(
        &self,
        orcid: &str,
        token: Option<&Token>,
        put_codes: &[PutCode],
    ) -> ClientResult<Bulk> {
        if put_codes.is_empty() || put_codes.len() > MAX_BULK {
            return Err(ClientError::Usage(format!(
                "a bulk read takes 1..={MAX_BULK} put-codes, got {}",
                put_codes.len()
            )));
        }
        let list: Vec<String> = put_codes.iter().map(|p| p.to_string()).collect();
        match self
            .get_doc(orcid, token, &format!("/works/{}", list.join(",")))?
            .root
        {
            Root::Bulk(b) => Ok(b),
            other => Err(unexpected("bulk", &other)),
        }
    }

    // ------------------------------------------------------------ search

    /// `GET /search/?q={query}&start={start}&rows={rows}`: one page of iDs
    /// matching a Solr query (e.g. `family-name:Garcia`,
    /// `doi-self:"10.1000/xyz"`). `rows` is 1..=[`MAX_SEARCH_ROWS`].
    ///
    /// Paced like every other request (one per 45 s to ORCID). Search is for
    /// finding a record, not a bulk-export path: for bulk data use the
    /// Public Data File and the `dump` reader.
    ///
    /// ```no_run
    /// use yorc::blocking::{Access, Client, Config};
    ///
    /// let client = Client::new(Config::new(Access::Public));
    /// let page = client.search("family-name:Carberry AND given-names:Josiah", 0, 10, None)?;
    /// for id in &page.results {
    ///     println!("{:?}", id.path);
    /// }
    /// # Ok::<(), yorc::blocking::ClientError>(())
    /// ```
    pub fn search(
        &self,
        query: &str,
        start: u32,
        rows: u32,
        token: Option<&Token>,
    ) -> ClientResult<Search> {
        let rest = search_rest("search", query, start, rows)?;
        match self.get_url(&rest, token)?.root {
            Root::Search(s) => Ok(s),
            other => Err(unexpected("search", &other)),
        }
    }

    /// `GET /expanded-search/?q=...`: [`Client::search`] with names, emails
    /// and institutions in each result.
    pub fn expanded_search(
        &self,
        query: &str,
        start: u32,
        rows: u32,
        token: Option<&Token>,
    ) -> ClientResult<ExpandedSearch> {
        let rest = search_rest("expanded-search", query, start, rows)?;
        match self.get_url(&rest, token)?.root {
            Root::ExpandedSearch(s) => Ok(s),
            other => Err(unexpected("expanded-search", &other)),
        }
    }
}

fn search_rest(endpoint: &str, query: &str, start: u32, rows: u32) -> ClientResult<String> {
    if query.trim().is_empty() {
        return Err(ClientError::Usage("a search needs a query".into()));
    }
    if rows == 0 || rows > MAX_SEARCH_ROWS {
        return Err(ClientError::Usage(format!(
            "a search page takes 1..={MAX_SEARCH_ROWS} rows, got {rows}"
        )));
    }
    Ok(format!(
        "/{endpoint}/?q={}&start={start}&rows={rows}",
        encode_component(query)
    ))
}

fn error_doc(text: &str) -> Option<OrcidError> {
    let doc = if text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('{')
    {
        json_doc("error", text)
    } else {
        xml::from_str(text).ok()
    };
    doc.and_then(|d| match d.root {
        Root::Error(e) => Some(e),
        _ => None,
    })
}

fn api_error(ans: &Answer) -> ClientError {
    let text = String::from_utf8_lossy(&ans.bytes);
    ClientError::Api {
        status: ans.status,
        error: error_doc(&text),
        body: text.into(),
    }
}

/// Maps ORCID's token response (`access_token`, `token_type`, `refresh_token`,
/// `expires_in`, `scope`) onto a [`Token`] of `realm` granted at `now`; the
/// scope is `/read-public` when the response names none.
fn token_from_response(json: &Value, realm: Realm, now: SystemTime) -> ClientResult<Token> {
    let text = |k: &str| {
        json.get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let access_token = text("access_token")
        .ok_or_else(|| ClientError::Decode("token response has no access_token".into()))?;
    if let Some(t) = text("token_type") {
        if !t.eq_ignore_ascii_case("bearer") {
            return Err(ClientError::Decode(format!(
                "token_type {t:?} is not bearer"
            )));
        }
    }
    let expires_in = json
        .get("expires_in")
        .and_then(|v| v.as_u64().or_else(|| v.as_str()?.parse().ok()));
    Ok(Token {
        access_token,
        refresh_token: text("refresh_token"),
        scope: text("scope").unwrap_or_else(|| SCOPE_READ_PUBLIC.to_string()),
        granted_at: now,
        expires_at: expires_in.and_then(|s| now.checked_add(Duration::from_secs(s))),
        realm,
    })
}

fn unexpected(wanted: &str, got: &Root) -> ClientError {
    let name = match got {
        Root::Unknown(n) => n.clone(),
        other => format!("{other:?}")
            .split('(')
            .next()
            .unwrap_or_default()
            .to_string(),
    };
    ClientError::Decode(format!("expected a {wanted} document, got {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(api: &str) -> Access {
        Access::Custom {
            api: api.into(),
            oauth: "http://127.0.0.1:9/oauth".into(),
        }
    }

    #[test]
    fn urls() {
        assert_eq!(Access::Public.api_base(), "https://pub.orcid.org/v3.0");
        assert_eq!(Access::Public.oauth_base(), "https://orcid.org/oauth");
        assert_eq!(
            custom("http://127.0.0.1:9/v3.0/").api_base(),
            "http://127.0.0.1:9/v3.0"
        );
        assert_eq!(Access::Public.realm(), Realm::Production);
        assert_eq!(custom("http://127.0.0.1:9/v3.0").realm(), Realm::Custom);
    }

    #[test]
    fn host_sets_are_closed() {
        let any = |a: &Access, u: &str| {
            [Purpose::Read, Purpose::OAuth]
                .into_iter()
                .any(|p| a.permits(p, u))
        };
        // Public: pub.orcid.org for reads, orcid.org/oauth for the token call
        assert!(Access::Public.permits(Purpose::Read, "https://pub.orcid.org/v3.0/x"));
        assert!(Access::Public.permits(Purpose::OAuth, "https://orcid.org/oauth/token"));
        assert!(!Access::Public.permits(Purpose::OAuth, "https://pub.orcid.org/v3.0/x"));
        assert!(!Access::Public.permits(Purpose::Read, "https://orcid.org/oauth/token"));
        // nothing else of ORCID's: no Member API, no Sandbox
        for u in [
            "https://api.orcid.org/v3.0/x",
            "https://sandbox.orcid.org/oauth/token",
            "https://pub.sandbox.orcid.org/v3.0/x",
            "https://api.sandbox.orcid.org/v3.0/x",
            "https://orcid.org/v3.0/x",
        ] {
            assert!(!any(&Access::Public, u), "{u}");
        }
    }

    #[test]
    fn forged_urls_are_refused() {
        let p = Access::Public;
        for u in [
            "https://pub.orcid.org.evil.example/v3.0/x",
            "https://pub.orcid.org@evil.example/v3.0/x",
            "https://pub.orcid.org:8443/v3.0/x",
            "http://pub.orcid.org/v3.0/x",
            "https://pub.orcid.org/v3.0evil/x",
            "https://PUB.orcid.org/v3.0/x",
            "https://pub.orcid.org/v3.0/x y",
            "https://pub.orcid.org/v3.0/x\r\nHost: orcid.org",
            "https://pub.orcid.org/v3.0\\@orcid.org/",
            "",
        ] {
            assert!(!p.permits(Purpose::Read, u), "{u:?}");
            assert!(!p.permits(Purpose::OAuth, u), "{u:?}");
        }
        assert!(p.permits(Purpose::Read, "https://pub.orcid.org/v3.0?x=1"));
        let c = custom("http://127.0.0.1:9/v3.0/");
        assert!(c.permits(Purpose::Read, "http://127.0.0.1:9/v3.0/x"));
        assert!(!c.permits(Purpose::Read, "http://127.0.0.1:90/v3.0/x"));
        assert!(!c.permits(Purpose::Read, "https://pub.orcid.org/v3.0/x"));
        // a Custom aimed at an ORCID host reaches nothing: ORCID only
        // through Public
        for api in [
            "https://pub.orcid.org/v3.0",
            "https://api.orcid.org/v3.0",
            "https://api.sandbox.orcid.org/v3.0",
            "https://pub.sandbox.orcid.org/v3.0",
        ] {
            let c = Access::Custom {
                api: api.into(),
                oauth: "https://sandbox.orcid.org/oauth".into(),
            };
            assert!(!c.permits(Purpose::Read, &format!("{api}/x")), "{api}");
            assert!(
                !c.permits(Purpose::OAuth, "https://sandbox.orcid.org/oauth/token"),
                "{api}"
            );
        }
    }

    #[test]
    fn a_forged_url_is_refused_by_the_send_path() {
        // the check sits in `exchange`, under every request: no listener is
        // needed, because nothing is sent
        let c = Client::with_limiter(
            Config::new(custom("http://127.0.0.1:9/v3.0")),
            Arc::new(RateLimiter::new(1000.0)),
        );
        for (method, purpose, url) in [
            (
                Method::Get,
                Purpose::Read,
                "https://pub.orcid.org/v3.0/x/record",
            ),
            (
                Method::Get,
                Purpose::Read,
                "https://api.orcid.org/v3.0/x/works",
            ),
            (
                Method::Post,
                Purpose::OAuth,
                "https://orcid.org/oauth/token",
            ),
            (Method::Get, Purpose::Read, "http://127.0.0.1:9/oauth/token"),
        ] {
            let r = c.exchange(&Request {
                method,
                purpose,
                url,
                accept: ORCID_XML,
                content_type: None,
                bearer: None,
                body: None,
            });
            assert!(matches!(r, Err(ClientError::Usage(_))), "{url}");
        }
    }

    #[test]
    fn tokens_are_checked() {
        let rp = super::Token::new(Realm::Production, "t", "/read-public");
        assert!(Access::Public.check_token(&rp).is_ok());
        // another realm
        let mock = super::Token::new(Realm::Custom, "t", "/read-public");
        assert!(matches!(
            Access::Public.check_token(&mock),
            Err(ClientError::Usage(_))
        ));
        let mock_access = custom("http://127.0.0.1:9/v3.0");
        assert!(mock_access.check_token(&rp).is_err());
        // empty, and any scope but /read-public
        for (token, scope) in [
            (" ", "/read-public"),
            ("t", "/authenticate"),
            ("t", "/read-limited"),
            ("t", "openid"),
            ("t", ""),
        ] {
            let t = super::Token::new(Realm::Production, token, scope);
            assert!(
                matches!(Access::Public.check_token(&t), Err(ClientError::Usage(_))),
                "{token:?} {scope}"
            );
        }
        let both = super::Token::new(Realm::Production, "t", "/authenticate /read-public");
        assert!(Access::Public.check_token(&both).is_ok());
    }

    #[test]
    fn public_is_clamped_to_45_seconds() {
        assert_eq!(ORCID_INTERVAL, Duration::from_millis(45_000));
        assert_eq!(RateLimiter::default().interval(), ORCID_INTERVAL);
        assert_eq!(RateLimiter::new(f64::NAN).interval(), ORCID_INTERVAL);
        assert_eq!(
            RateLimiter::new(12.0).interval(),
            Duration::from_secs_f64(1.0 / 12.0)
        );
        // a fast limiter is clamped; a slower one is kept
        let fast = Client::with_limiter(
            Config::new(Access::Public),
            Arc::new(RateLimiter::new(1000.0)),
        );
        assert_eq!(fast.min_interval(), ORCID_INTERVAL);
        assert_eq!(fast.max_attempts(), ORCID_MAX_ATTEMPTS);
        let mut cfg = Config::new(Access::Public);
        cfg.max_retries = 10;
        let slow = Client::with_limiter(
            cfg,
            Arc::new(RateLimiter::with_interval(Duration::from_secs(60))),
        );
        assert_eq!(slow.min_interval(), Duration::from_secs(60));
        assert_eq!(slow.max_attempts(), 3, "at most 3 attempts to ORCID");
        let public = Client::new(Config::new(Access::Public));
        assert!(Arc::ptr_eq(
            public.orcid.as_ref().unwrap(),
            &RateLimiter::orcid()
        ));
        let mock = Client::with_limiter(
            Config::new(custom("http://127.0.0.1:9/v3.0")),
            Arc::new(RateLimiter::new(1000.0)),
        );
        assert!(mock.min_interval() < Duration::from_millis(2));
        assert!(mock.orcid.is_none());
        // a Custom pointed at an ORCID host is paced like Public (and its
        // requests are refused anyway)
        for api in [
            "https://pub.orcid.org/v3.0",
            "https://API.Sandbox.orcid.org:443/v3.0",
            "http://user@orcid.org./v3.0",
        ] {
            let a = custom(api);
            assert!(a.is_orcid(), "{api}");
            let c = Client::with_limiter(Config::new(a), Arc::new(RateLimiter::new(1000.0)));
            assert_eq!(c.min_interval(), ORCID_INTERVAL, "{api}");
        }
        assert!(!is_orcid_host("http://127.0.0.1:9/v3.0"));
        assert!(!is_orcid_host("https://notorcid.org/v3.0"));
        assert!(!is_orcid_host("https://figshare.com/orcid.org"));
    }

    #[test]
    fn token_response() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let json: Value = serde_json::from_str(
            r#"{"access_token":"4bed1e13-7792-4129-9f07-aaf7b88ba88f","token_type":"bearer",
                "refresh_token":"2d76d8d0-6fd6-426b-a017-61e0ceda0ad2","expires_in":631138518,
                "scope":"/read-public","name":null,"orcid":null}"#,
        )
        .unwrap();
        let t = token_from_response(&json, Realm::Production, now).unwrap();
        assert_eq!(t.access_token, "4bed1e13-7792-4129-9f07-aaf7b88ba88f");
        assert_eq!(t.scope, "/read-public");
        assert_eq!(t.realm, Realm::Production);
        assert_eq!(t.expires_at, Some(now + Duration::from_secs(631_138_518)));
        assert!(!t.is_expired(now));
        assert!(t.is_expired(now + Duration::from_secs(631_138_518)));
        assert!(
            !format!("{t:?}").contains("4bed1e13"),
            "Debug must not print the token"
        );
        assert!(
            !format!("{t:?}").contains("2d76d8d0"),
            "Debug must not print the refresh token"
        );
        // no scope in the answer: /read-public; no access_token: an error
        let cc = serde_json::json!({"access_token": "x", "token_type": "bearer"});
        let t = token_from_response(&cc, Realm::Custom, now).unwrap();
        assert_eq!(t.scope, "/read-public");
        assert_eq!(t.realm, Realm::Custom);
        let none = serde_json::json!({"token_type": "bearer"});
        assert!(matches!(
            token_from_response(&none, Realm::Custom, now),
            Err(ClientError::Decode(_))
        ));
        let mac: Value =
            serde_json::from_str(r#"{"access_token":"x","token_type":"mac"}"#).unwrap();
        assert!(matches!(
            token_from_response(&mac, Realm::Custom, now),
            Err(ClientError::Decode(_))
        ));
    }

    #[test]
    fn limiter_paces() {
        let lim = RateLimiter::new(100.0);
        let start = Instant::now();
        for _ in 0..6 {
            lim.acquire();
        }
        assert!(start.elapsed() >= Duration::from_millis(45));
        // a minimum spacing larger than the interval wins
        let lim = RateLimiter::new(1000.0);
        let start = Instant::now();
        for _ in 0..3 {
            lim.acquire_spaced(Duration::from_millis(30));
        }
        assert!(start.elapsed() >= Duration::from_millis(55));
    }
}

//! The Public API client against a local mock server (std::net::TcpListener):
//! methods, paths, headers and bodies are asserted; canned responses include
//! 429 + Retry-After, 5xx, error-3.0 bodies and the OAuth token endpoint
//! (the optional `/read-public` token). The client only reads ORCID's
//! Public API; it has no Member API or Sandbox access.
//!
//! Mock servers are local, so every client here is `Access::Custom`, with a
//! fast limiter of its own (Custom is not paced by the 45 s ORCID limiter).
//! `Public`'s host set, the token checks and the 45 s clamp are tested as
//! pure functions in src/blocking.rs.
#![cfg(feature = "blocking")]

mod support;

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use support::*;
use yorc::blocking::{Access, Client, ClientError, Config, RateLimiter, Realm, Token};
use yorc::model::*;

const ID: &str = "0000-0002-1825-0097";
const OTHER: &str = "0000-0001-5109-3700";

#[derive(Debug, Clone)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
    at: Instant,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

struct Reply {
    status: u16,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: String,
}

fn reply(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "application/vnd.orcid+xml",
        headers: Vec::new(),
        body: body.to_string(),
    }
}

fn json_reply(status: u16, body: &str) -> Reply {
    Reply {
        content_type: "application/json",
        ..reply(status, body)
    }
}

fn with_header(mut r: Reply, k: &'static str, v: &str) -> Reply {
    r.headers.push((k, v.to_string()));
    r
}

struct Mock {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    handle: Option<JoinHandle<()>>,
}

impl Mock {
    /// Serves `script` in order, one connection per request (`Connection: close`).
    fn start(script: Vec<Reply>) -> Mock {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        let handle = thread::spawn(move || {
            for r in script {
                let (stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let mut parts = line.split_whitespace();
                let method = parts.next().unwrap_or_default().to_string();
                let path = parts.next().unwrap_or_default().to_string();
                let mut headers = Vec::new();
                let mut length = 0usize;
                loop {
                    let mut h = String::new();
                    reader.read_line(&mut h).unwrap();
                    let h = h.trim_end();
                    if h.is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':') {
                        if k.eq_ignore_ascii_case("content-length") {
                            length = v.trim().parse().unwrap();
                        }
                        headers.push((k.trim().to_string(), v.trim().to_string()));
                    }
                }
                let mut body = vec![0; length];
                reader.read_exact(&mut body).unwrap();
                log.lock().unwrap().push(Seen {
                    method,
                    path,
                    headers,
                    body: String::from_utf8(body).unwrap(),
                    at: Instant::now(),
                });
                let mut out = stream;
                let mut head = format!(
                    "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: {}\r\n",
                    r.status,
                    r.body.len(),
                    r.content_type
                );
                for (k, v) in &r.headers {
                    head.push_str(&format!("{k}: {v}\r\n"));
                }
                head.push_str("\r\n");
                out.write_all(head.as_bytes()).unwrap();
                out.write_all(r.body.as_bytes()).unwrap();
                out.flush().unwrap();
            }
        });
        Mock {
            base,
            seen,
            handle: Some(handle),
        }
    }

    fn access(&self) -> Access {
        Access::Custom {
            api: format!("{}/v3.0", self.base),
            oauth: format!("{}/oauth", self.base),
        }
    }

    fn client(&self) -> Client {
        let mut cfg = Config::new(self.access());
        cfg.backoff_base = Duration::from_millis(10);
        cfg.max_retries = 3;
        cfg.timeout = Duration::from_secs(10);
        Client::with_limiter(cfg, Arc::new(RateLimiter::new(1000.0)))
    }

    fn finish(mut self) -> Vec<Seen> {
        self.handle.take().unwrap().join().unwrap();
        self.seen.lock().unwrap().clone()
    }

    /// What the server saw so far (for scripts that are not run to the end).
    fn seen_now(&self) -> usize {
        self.seen.lock().unwrap().len()
    }
}

fn read_sample(name: &str) -> String {
    fs::read_to_string(record_xsd_dir().join("samples/read_samples").join(name)).unwrap()
}

/// A `/read-public` token of the Custom realm.
fn tok() -> Token {
    Token::new(Realm::Custom, "tok", "/read-public")
}

fn assert_get(s: &Seen, path: &str) {
    assert_eq!(s.method, "GET");
    assert_eq!(s.path, format!("/v3.0{path}"));
    assert_eq!(s.header("accept"), Some("application/vnd.orcid+xml"));
    assert_eq!(s.header("authorization"), Some("Bearer tok"));
    assert!(s.body.is_empty());
}

// ---------------------------------------------------------------- reads

#[test]
fn reads() {
    let mock = Mock::start(vec![
        reply(200, &read_sample("record-3.0.xml")),
        reply(200, &read_sample("person-3.0.xml")),
        reply(200, &read_sample("works-3.0.xml")),
        reply(200, &read_sample("work-full-3.0.xml")),
        reply(200, &read_sample("bulk-work-err.xml")),
        reply(200, &read_sample("educations-3.0.xml")),
    ]);
    let c = mock.client();
    let t = tok();
    let t = Some(&t);
    assert!(c.record(ID, t).unwrap().person.is_some());
    assert!(
        c.person(&format!("https://orcid.org/{ID}"), t)
            .unwrap()
            .name
            .is_some()
    );
    assert!(!c.works(ID, t).unwrap().groups.is_empty());
    assert!(c.work(ID, t, 123).unwrap().title.is_some());
    let b = c.works_bulk(ID, t, &[1, 2, 3]).unwrap();
    assert!(b.items.iter().any(|i| matches!(i, BulkItem::Error(_))));
    assert!(matches!(
        c.section(ID, t, "educations").unwrap().root,
        Root::Affiliations(_)
    ));
    let seen = mock.finish();
    assert_eq!(seen.len(), 6);
    assert_get(&seen[0], &format!("/{ID}/record"));
    assert_get(&seen[1], &format!("/{ID}/person"));
    assert_get(&seen[2], &format!("/{ID}/works"));
    assert_get(&seen[3], &format!("/{ID}/work/123"));
    assert_get(&seen[4], &format!("/{ID}/works/1,2,3"));
    assert_get(&seen[5], &format!("/{ID}/educations"));
}

#[test]
fn any_public_record_without_a_token() {
    let mock = Mock::start(vec![
        reply(200, &read_sample("record-3.0.xml")),
        reply(200, &read_sample("record-3.0.xml")),
    ]);
    let c = mock.client();
    assert!(c.record(OTHER, None).is_ok());
    let empty = Token::new(Realm::Custom, "", "/read-public");
    assert!(c.record_raw(&OTHER.replace('-', ""), Some(&empty)).is_ok());
    let seen = mock.finish();
    assert_eq!(seen.len(), 2);
    for s in &seen {
        assert_eq!(s.path, format!("/v3.0/{OTHER}/record"));
        assert_eq!(s.header("accept"), Some("application/vnd.orcid+xml"));
        assert!(
            s.header("authorization").is_none(),
            "no token, no Authorization"
        );
    }
}

#[test]
fn bad_arguments_are_refused_without_a_request() {
    let mock = Mock::start(Vec::new());
    let c = mock.client();
    let t = tok();
    let t = Some(&t);
    for r in [
        c.record("garbage", t).map(|_| ()),
        c.record("0000-0002-1825-0098", t).map(|_| ()),
        c.record_raw("garbage", t).map(|_| ()),
        c.work("", t, 1).map(|_| ()),
        c.section(ID, t, "../0000-0001-5109-3700/record")
            .map(|_| ()),
        c.section(ID, t, "").map(|_| ()),
        c.works_bulk(ID, t, &[]).map(|_| ()),
        c.works_bulk(ID, t, &[1; 101]).map(|_| ()),
    ] {
        assert!(matches!(r, Err(ClientError::Usage(_))), "{r:?}");
    }
    assert_eq!(mock.finish().len(), 0, "nothing may reach ORCID");
}

#[test]
fn reads_retry_5xx_honouring_retry_after() {
    let mock = Mock::start(vec![
        reply(500, "oops"),
        with_header(reply(503, "busy"), "Retry-After", "1"),
        reply(200, &read_sample("record-3.0.xml")),
    ]);
    let t = tok();
    assert!(mock.client().record(ID, Some(&t)).is_ok());
    let seen = mock.finish();
    assert_eq!(seen.len(), 3);
    let gap = seen[2].at.duration_since(seen[1].at);
    assert!(
        gap >= Duration::from_millis(950),
        "Retry-After: 1 not honoured ({gap:?})"
    );
}

#[test]
fn a_429_stops_and_pauses_until_retry_after() {
    let mock = Mock::start(vec![
        with_header(reply(429, ""), "Retry-After", "1"),
        reply(200, &read_sample("record-3.0.xml")),
    ]);
    let c = mock.client();
    let t = tok();
    assert!(matches!(
        c.record(ID, Some(&t)),
        Err(ClientError::Api { status: 429, .. })
    ));
    assert_eq!(mock.seen_now(), 1, "a 429 is not retried");
    // the next request (even another call) waits for Retry-After
    assert!(c.record(ID, Some(&t)).is_ok());
    let seen = mock.finish();
    assert_eq!(seen.len(), 2);
    let gap = seen[1].at.duration_since(seen[0].at);
    assert!(gap >= Duration::from_millis(950), "{gap:?}");
}

#[test]
fn reads_give_up_after_max_retries_and_parse_errors() {
    let mock = Mock::start((0..4).map(|_| reply(500, "")).collect());
    let t = tok();
    assert!(matches!(
        mock.client().record(ID, Some(&t)),
        Err(ClientError::Api { status: 500, .. })
    ));
    assert_eq!(mock.finish().len(), 4, "1 attempt + 3 retries");

    let mock = Mock::start(vec![reply(404, &read_sample("error-3.0.xml"))]);
    match mock.client().work(ID, Some(&t), 9) {
        Err(ClientError::Api {
            status: 404,
            error: Some(e),
            ..
        }) => {
            assert_eq!(
                e.developer_message.as_deref(),
                Some("error:developer-message")
            );
            assert_eq!(e.more_info.as_deref(), Some("http://tempuri.org"));
        }
        other => panic!("{other:?}"),
    }
    mock.finish();

    let mock = Mock::start(vec![reply(200, &read_sample("person-3.0.xml"))]);
    assert!(matches!(
        mock.client().record(ID, Some(&t)),
        Err(ClientError::Decode(_))
    ));
    mock.finish();
}

#[test]
fn read_public_token() {
    let mock = Mock::start(vec![
        json_reply(
            200,
            r#"{"access_token":"4bed1e13-7792-4129-9f07-aaf7b88ba88f","token_type":"bearer",
                "refresh_token":"2d76d8d0-6fd6-426b-a017-61e0ceda0ad2","expires_in":631138518,
                "scope":"/read-public","name":null,"orcid":null}"#,
        ),
        json_reply(
            401,
            r#"{"error":"invalid_client","error_description":"Bad client credentials"}"#,
        ),
        reply(200, &read_sample("record-3.0.xml")),
    ]);
    let c = mock.client();
    // 2-legged: a /read-public token for the client, of the client's realm
    let p = c
        .read_public_token("APP-1234567890ABCDEF", "s3cret")
        .unwrap();
    assert_eq!(p.scope, "/read-public");
    assert_eq!(p.access_token, "4bed1e13-7792-4129-9f07-aaf7b88ba88f");
    assert_eq!(p.realm, Realm::Custom);
    assert!(p.expires_at.is_some());

    match c.read_public_token("APP-1234567890ABCDEF", "wrong") {
        Err(ClientError::OAuth { error, description }) => {
            assert_eq!(error, "invalid_client");
            assert_eq!(description.as_deref(), Some("Bad client credentials"));
        }
        other => panic!("{other:?}"),
    }

    // the token is sent with reads
    assert!(c.record(ID, Some(&p)).is_ok());

    let seen = mock.finish();
    assert_eq!(seen.len(), 3);
    for t in &seen[..2] {
        assert_eq!(t.method, "POST");
        assert_eq!(t.path, "/oauth/token");
        assert_eq!(t.header("accept"), Some("application/json"));
        assert_eq!(
            t.header("content-type"),
            Some("application/x-www-form-urlencoded")
        );
        assert!(t.header("authorization").is_none());
    }
    assert_eq!(
        seen[0].body,
        "client_id=APP-1234567890ABCDEF&client_secret=s3cret&grant_type=client_credentials\
         &scope=%2Fread-public"
    );
    assert_eq!(seen[2].path, format!("/v3.0/{ID}/record"));
    assert_eq!(
        seen[2].header("authorization"),
        Some("Bearer 4bed1e13-7792-4129-9f07-aaf7b88ba88f")
    );
}

#[test]
fn a_token_request_is_not_retried() {
    // one answer scripted: a retry would find no server and fail differently
    let mock = Mock::start(vec![json_reply(503, "{}")]);
    let r = mock.client().read_public_token("APP-1", "s");
    assert!(matches!(r, Err(ClientError::OAuth { .. })), "{r:?}");
    assert_eq!(mock.finish().len(), 1, "POST is never retried");
}

#[test]
fn shared_limiter_paces_across_clients() {
    let mock = Mock::start(
        (0..4)
            .map(|_| reply(200, &read_sample("record-3.0.xml")))
            .collect(),
    );
    let lim = Arc::new(RateLimiter::new(20.0)); // 50 ms apart
    let mut cfg = Config::new(mock.access());
    cfg.timeout = Duration::from_secs(10);
    let a = Client::with_limiter(cfg.clone(), Arc::clone(&lim));
    let b = Client::with_limiter(cfg, Arc::clone(&lim));
    let t = tok();
    let start = Instant::now();
    for c in [&a, &b, &a, &b] {
        c.record(ID, Some(&t)).unwrap();
    }
    assert!(start.elapsed() >= Duration::from_millis(140));
    mock.finish();
}

#[test]
fn record_raw_returns_the_exact_bytes_and_error_documents() {
    // odd but valid bytes: a BOM, CRLF line ends, trailing spaces, non-ASCII
    let exact = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<record:record xmlns:record=\"http://www.orcid.org/ns/record\" path=\"/0000-0002-1825-0097\">Ünïcode  </record:record>\r\n";
    let deprecated = read_sample("error-3.0.xml");
    let mock = Mock::start(vec![
        reply(200, exact),
        with_header(
            reply(301, &deprecated),
            "Location",
            "https://pub.orcid.org/v3.0/0000-0001-5109-3700/record",
        ),
        reply(409, &deprecated),
        reply(404, &deprecated),
    ]);
    let c = mock.client();
    let t = tok();
    let t = Some(&t);
    let (status, body) = c.record_raw(ID, t).unwrap();
    assert_eq!(status, 200);
    assert_eq!(body, exact.as_bytes(), "byte for byte");
    let (status, body) = c.record_raw(&format!("https://orcid.org/{ID}"), t).unwrap();
    assert_eq!(
        status, 301,
        "a deprecated record's error document, redirect not followed"
    );
    assert_eq!(body, deprecated.as_bytes());
    assert_eq!(c.record_raw(ID, t).unwrap().0, 409, "locked");
    match c.record_raw(ID, t) {
        Err(ClientError::Api {
            status: 404, error, ..
        }) => assert!(error.is_some()),
        other => panic!("{other:?}"),
    }
    let seen = mock.finish();
    assert_eq!(seen.len(), 4, "no redirect followed");
    for s in &seen {
        assert_get(s, &format!("/{ID}/record"));
    }
}

const EXPANDED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<expanded-search:expanded-search num-found="2" xmlns:expanded-search="http://www.orcid.org/ns/expanded-search">
  <expanded-search:expanded-result>
    <expanded-search:orcid-id>0000-0002-1825-0097</expanded-search:orcid-id>
    <expanded-search:given-names>Josiah</expanded-search:given-names>
    <expanded-search:family-names>Carberry</expanded-search:family-names>
    <expanded-search:institution-name>Brown University</expanded-search:institution-name>
  </expanded-search:expanded-result>
</expanded-search:expanded-search>
"#;

#[test]
fn search_pages() {
    let mock = Mock::start(vec![
        reply(200, &read_sample("search-3.0.xml")),
        reply(200, EXPANDED),
        reply(200, &read_sample("record-3.0.xml")),
    ]);
    let c = mock.client();
    let t = tok();
    let t = Some(&t);
    let s = c
        .search("family-name:Carberry AND given-names:J*", 0, 1000, t)
        .unwrap();
    assert_eq!(s.num_found, Some(1));
    assert_eq!(s.results[0].path.as_deref(), Some("8888-8888-8888-8880"));
    let e = c.expanded_search("doi-self:\"10.1/x\"", 20, 10, t).unwrap();
    assert_eq!(e.num_found, Some(2));
    assert_eq!(e.results[0].family_names.as_deref(), Some("Carberry"));
    assert_eq!(e.results[0].institution_names, ["Brown University"]);
    assert!(matches!(
        c.search("x", 0, 10, t),
        Err(ClientError::Decode(_))
    ));
    let seen = mock.finish();
    assert_eq!(seen.len(), 3);
    assert_get(
        &seen[0],
        "/search/?q=family-name%3ACarberry%20AND%20given-names%3AJ%2A&start=0&rows=1000",
    );
    assert_get(
        &seen[1],
        "/expanded-search/?q=doi-self%3A%2210.1%2Fx%22&start=20&rows=10",
    );
}

#[test]
fn search_arguments_are_refused_without_a_request() {
    let mock = Mock::start(Vec::new());
    let c = mock.client();
    let t = tok();
    let t = Some(&t);
    for r in [
        c.search("", 0, 10, t).map(|_| ()),
        c.search("  ", 0, 10, t).map(|_| ()),
        c.search("x", 0, 0, t).map(|_| ()),
        c.search("x", 0, 1001, t).map(|_| ()),
        c.expanded_search("x", 0, 1001, t).map(|_| ()),
    ] {
        assert!(matches!(r, Err(ClientError::Usage(_))), "{r:?}");
    }
    assert_eq!(mock.finish().len(), 0, "nothing may reach ORCID");
}

// ---------------------------------------------------------------- tokens

#[test]
fn other_tokens_are_refused_without_a_request() {
    let mock = Mock::start(Vec::new());
    let c = mock.client();
    // a token of another realm, or with any scope but /read-public (no
    // sign-in or member-scoped tokens)
    for t in [
        Token::new(Realm::Production, "tok", "/read-public"),
        Token::new(Realm::Custom, "tok", "/authenticate"),
        Token::new(Realm::Custom, "tok", "/read-limited"),
        Token::new(Realm::Custom, "tok", "/activities/update"),
        Token::new(Realm::Custom, " ", "/read-public"),
    ] {
        for r in [
            c.record(ID, Some(&t)).map(|_| ()),
            c.record_raw(ID, Some(&t)).map(|_| ()),
            c.search("x", 0, 10, Some(&t)).map(|_| ()),
        ] {
            assert!(matches!(r, Err(ClientError::Usage(_))), "{t:?}: {r:?}");
        }
    }
    assert_eq!(mock.finish().len(), 0, "nothing may reach ORCID");
}

//! A path that could be read as another (an encoded separator or backslash, or a dot segment) is answered
//! 400 before any route, whoever sends it and wherever it seems to lead. It is sent as raw bytes, since an
//! HTTP client would resolve the dot segments first.
mod common;

use common::access::{access_token, behind_access, PUBLIC_HOST};
use common::Running;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The status line's code of a GET of `path` sent as written, through the public host name.
async fn status_of_raw(running: &Running, path: &str, jwt: Option<&str>) -> u16 {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", running.port))
        .await
        .unwrap();
    let access = jwt.map_or(String::new(), |jwt| {
        format!("cf-access-jwt-assertion: {jwt}\r\n")
    });
    let request = format!(
        "GET {path} HTTP/1.1\r\nhost: {PUBLIC_HOST}\r\ncf-ray: 1\r\n{access}connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).await.unwrap();
    answer
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("no status: {answer}"))
}

#[tokio::test]
async fn a_path_with_an_encoded_separator_or_a_dot_segment_is_refused_before_any_route() {
    let alice = behind_access(None).await;
    let owner = access_token("alice@example.com", |_| {});
    for path in [
        "/fed/..%2fapi/me",
        "/fed/..%2Fapi/session",
        "/fed/%2e%2e/api/me",
        "/fed/v1/../../api/me",
        "/fed/./v1/teams/acme/app/view",
        "/api%5cme",
        "/fed/v1/teams/acme%2fapp/view",
    ] {
        for jwt in [None, Some(owner.as_str())] {
            assert_eq!(
                status_of_raw(&alice, path, jwt).await,
                400,
                "{path}, signed in: {}",
                jwt.is_some()
            );
        }
    }
    assert_eq!(
        status_of_raw(&alice, "/api/me", Some(&owner)).await,
        200,
        "a plain path is read"
    );
    assert_eq!(
        status_of_raw(&alice, "/api/me", None).await,
        403,
        "and still needs a sign-in"
    );
}

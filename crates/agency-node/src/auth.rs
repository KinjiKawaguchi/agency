//! 署名つきの読み出し。見出しに アカウント、時刻、署名 を付け、受け取る台は組織の定義の鍵で確かめる。
use agency_proto::org::OrgDefinition;
use agency_proto::signed::read_text;
use agency_proto::{clock, AccountKey, Id, Kind, Person};
use axum::http::{HeaderMap, StatusCode};

pub const ACCOUNT: &str = "x-agency-account";
pub const DATE: &str = "x-agency-date";
pub const SIGNATURE: &str = "x-agency-signature";
/// 時計のずれと、古い署名の使い回しを断る幅
pub const SKEW_SECONDS: i64 = 300;

pub fn signed_headers(key: &AccountKey, account: &Id, method: &str, path: &str) -> Vec<(&'static str, String)> {
    let date = clock::format(clock::now());
    let signature = key.sign(&Kind::Read.message(&read_text(method, path, &date, account)));
    vec![(ACCOUNT, account.to_string()), (DATE, date), (SIGNATURE, signature)]
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> &'a str {
    headers.get(name).and_then(|value| value.to_str().ok()).unwrap_or("")
}

/// 読み出しの署名を確かめ、所属する人を返す。所属していなければ 404 (あることも見せない)
pub fn verify_read<'a>(definition: &'a OrgDefinition, headers: &HeaderMap, method: &str, path: &str) -> Result<&'a Person, (StatusCode, String)> {
    let unauthorized = |message: &str| (StatusCode::UNAUTHORIZED, message.to_owned());
    let account = Id::parse(header(headers, ACCOUNT)).map_err(|_| unauthorized("アカウントの見出しが無い"))?;
    let date = header(headers, DATE);
    let at = clock::parse(date).ok_or_else(|| unauthorized("時刻の見出しの形が違う"))?;
    if (clock::now() - at).whole_seconds().abs() > SKEW_SECONDS {
        return Err(unauthorized("署名の時刻が古いか、先すぎる"));
    }
    let person = definition.member(&account).ok_or((StatusCode::NOT_FOUND, "見つからない".to_owned()))?;
    if !person.public_key.verify(&Kind::Read.message(&read_text(method, path, date, &account)), header(headers, SIGNATURE)) {
        return Err(unauthorized("署名が合わない"));
    }
    Ok(person)
}

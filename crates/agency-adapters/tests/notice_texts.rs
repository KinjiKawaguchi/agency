//! What reaches a lock screen or a browser's notification: every text someone else wrote (a team's label, a
//! request's title, a device's label, a refused decision's title, the words of a change) is sent with each
//! character that could hide or reorder how it reads replaced by `?`, through APNs and Web Push alike.
use agency_adapters::{apns, web_push};
use agency_core::notice::{Notice, NoticeKind};
use agency_proto::Id;
use serde_json::Value;

/// A line separator, a right-to-left override, a zero-width space, a line feed, and a byte-order mark.
const HIDING: &str = "Pay\u{2028}Approved by the owner \u{202E}gnp.exe\u{200B}\nok\u{FEFF}";
const SHOWN: &str = "Pay?Approved by the owner ?gnp.exe??ok?";

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn notices() -> Vec<Notice> {
    let kinds = vec![
        NoticeKind::Request {
            org: id("acme"),
            team: id("app"),
            request: "r-1".into(),
            team_label: Some(HIDING.into()),
            title: Some(HIDING.into()),
        },
        NoticeKind::Device {
            device: id("d-abc"),
            label: Some(HIDING.into()),
        },
        NoticeKind::Decision {
            org: id("acme"),
            team: id("app"),
            id: "e-1".into(),
            title: Some(HIDING.into()),
        },
        NoticeKind::RootChange {
            org: id("acme"),
            id: "h-1".into(),
            what: Some(HIDING.into()),
            waiting: 1,
        },
    ];
    kinds
        .into_iter()
        .map(|kind| Notice { kind, badge: 1 })
        .collect()
}

/// Every string anywhere in `value`.
fn strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => vec![text.clone()],
        Value::Array(items) => items.iter().flat_map(strings).collect(),
        Value::Object(fields) => fields.values().flat_map(strings).collect(),
        _ => vec![],
    }
}

fn holds_nothing_hidden(what: &str, payload: &Value) {
    let all = strings(payload);
    for text in &all {
        assert!(
            !text.chars().any(agency_proto::org::is_hidden_char),
            "{what}: {text:?} in {payload}"
        );
    }
}

#[test]
fn a_notice_is_sent_with_nothing_that_hides_or_reorders_a_line() {
    for notice in notices() {
        holds_nothing_hidden("APNs", &apns::payload(&notice, None));
        holds_nothing_hidden("Web Push", &web_push::payload(&notice));
    }
}

#[test]
fn the_rest_of_a_text_is_kept_as_written() {
    let notice = &notices()[0];
    let apns = apns::payload(notice, None);
    assert_eq!(apns["aps"]["alert"]["body"], SHOWN, "{apns}");
    let web = web_push::payload(notice);
    assert!(strings(&web).contains(&SHOWN.to_owned()), "{web}");
}

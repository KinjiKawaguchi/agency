//! The command runtime's `tickets` read: asked only of a runtime that lists it, checked as the team's own
//! answer, served printable, and kept for a while.
mod common;

use agency_adapters::runtime::CommandRuntime;
use agency_core::ports::TeamRuntime;
use common::*;
use serde_json::{json, Value};
use std::time::Duration;

fn listing_tickets() -> Fake {
    let fake = Fake::new();
    fake.answer(
        "describe",
        r#"{"result":{"runtimes":["agency-runtime/1"],"ops":["describe","view","decide","tickets"],"label":"App"}}"#,
        0,
    );
    fake.answer("tickets", &format!(r#"{{"result":{TICKETS}}}"#), 0);
    fake
}

fn answering(fake: &Fake, edit: impl FnOnce(&mut Value)) {
    let mut tickets: Value = serde_json::from_str(TICKETS).unwrap();
    edit(&mut tickets);
    fake.answer("tickets", &json!({ "result": tickets }).to_string(), 0);
}

fn unkept() -> CommandRuntime {
    CommandRuntime::default().keeping_tickets(Duration::ZERO, Duration::ZERO)
}

#[tokio::test]
async fn a_runtime_that_does_not_list_tickets_is_not_run_for_them() {
    let fake = Fake::new();
    let error = CommandRuntime::default()
        .tickets(&fake.team("app"))
        .await
        .unwrap_err();
    assert_eq!(error.status, 404);
    assert_eq!(fake.read("calls"), "describe\n");
}

#[tokio::test]
async fn reads_the_tickets_of_a_runtime_that_lists_them() {
    let fake = listing_tickets();
    let tickets = CommandRuntime::default()
        .tickets(&fake.team("app"))
        .await
        .unwrap();
    assert_eq!(tickets, serde_json::from_str(TICKETS).unwrap());
    assert_eq!(fake.read("calls"), "describe\ntickets\n");
    let asked: Value = serde_json::from_str(&fake.read("last-tickets.json")).unwrap();
    assert_eq!(
        asked,
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "tickets" })
    );
}

#[tokio::test]
async fn what_the_board_s_people_wrote_is_served_printable() {
    let fake = listing_tickets();
    answering(&fake, |tickets| {
        tickets["withoutEpic"][0]["title"] = json!("Pay\u{202E}later");
    });
    let tickets = unkept().tickets(&fake.team("app")).await.unwrap();
    assert_eq!(tickets.without_epic[0].title, "Pay?later");
}

#[tokio::test]
async fn an_answer_of_another_team_or_that_fails_the_checks_is_refused() {
    let fake = listing_tickets();
    let error = unkept().tickets(&fake.team("other")).await.unwrap_err();
    assert_eq!(error.status, 503);
    assert!(error.message.contains("acme/app"), "{}", error.message);

    answering(&fake, |tickets| {
        let child = tickets["epics"][0]["children"][0].clone();
        tickets["withoutEpic"].as_array_mut().unwrap().push(child);
    });
    let error = unkept().tickets(&fake.team("app")).await.unwrap_err();
    assert_eq!(
        (error.status, error.message.as_str()),
        (503, "the team runtime reported app#12 twice")
    );

    fake.answer("tickets", "not json", 1);
    assert_eq!(
        unkept()
            .tickets(&fake.team("app"))
            .await
            .unwrap_err()
            .status,
        503
    );
}

#[tokio::test]
async fn tickets_are_kept_for_a_while_and_read_again_after_it() {
    let fake = listing_tickets();
    let team = fake.team("app");
    let runs = || fake.read("calls").matches("tickets").count();

    let runtimes = CommandRuntime::default();
    runtimes.tickets(&team).await.unwrap();
    runtimes.tickets(&team).await.unwrap();
    assert_eq!(runs(), 1, "a young answer is served as it is");
    assert_eq!(runtimes.tickets_reads().fresh, 1);

    let unkept = unkept();
    unkept.tickets(&team).await.unwrap();
    unkept.tickets(&team).await.unwrap();
    assert_eq!(runs(), 3, "with nothing kept, each read runs the command");
}

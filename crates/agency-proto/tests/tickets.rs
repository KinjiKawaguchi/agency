//! The tickets contract: the shared sample answer, the `tickets` runtime op, and the checks a node makes
//! before it uses an answer.
use agency_proto::runtime::{Describe, TICKETS_OP};
use agency_proto::tickets::{
    OpenPullRequest, Priority, Tickets, MAX_EPICS, MAX_PULL_REQUESTS, MAX_TICKETS,
};
use agency_proto::{Id, RuntimeCall, RuntimeRequest};
use serde_json::{json, Value};

const SAMPLE: &str = include_str!("../../../testdata/tickets.sample.json");

fn id(raw: &str) -> Id {
    Id::parse(raw).unwrap()
}

fn sample() -> Tickets {
    serde_json::from_str(SAMPLE).unwrap()
}

fn check(tickets: &Tickets) -> Result<(), String> {
    tickets.check(&id("acme"), &id("app"))
}

fn changed(edit: impl FnOnce(&mut Tickets)) -> Tickets {
    let mut tickets = sample();
    edit(&mut tickets);
    tickets
}

#[test]
fn the_sample_round_trips_and_passes_the_checks() {
    let value: Value = serde_json::from_str(SAMPLE).unwrap();
    assert_eq!(serde_json::to_value(sample()).unwrap(), value);
    assert_eq!(check(&sample()), Ok(()));
}

#[test]
fn every_ticket_is_listed_once_epic_first_then_its_children_then_those_in_no_epic() {
    let refs: Vec<_> = sample()
        .all()
        .iter()
        .map(|ticket| ticket.reference.clone())
        .collect();
    assert_eq!(
        refs,
        ["app#10", "app#12", "app#13", "site#7", "app#20", "app#21"]
    );
}

#[test]
fn the_op_is_asked_with_no_input_and_named_in_describe() {
    let asked = RuntimeRequest::new(id("acme"), id("app"), RuntimeCall::Tickets);
    let expected =
        json!({ "runtime": "agency-runtime/1", "org": "acme", "team": "app", "op": "tickets" });
    assert_eq!(serde_json::to_value(&asked).unwrap(), expected);
    assert_eq!(
        serde_json::from_value::<RuntimeRequest>(expected).unwrap(),
        asked
    );
    let describe: Describe = serde_json::from_value(json!({
        "runtimes": ["agency-runtime/1"], "ops": ["describe", "view", "decide", "tickets"], "label": "App"
    }))
    .unwrap();
    assert!(describe.supports(TICKETS_OP));
}

#[test]
fn priorities_are_the_five_words_and_nothing_else() {
    let words = [
        (Priority::Highest, "highest"),
        (Priority::High, "high"),
        (Priority::Medium, "medium"),
        (Priority::Low, "low"),
        (Priority::Deferred, "deferred"),
    ];
    for (priority, word) in words {
        assert_eq!(serde_json::to_value(priority).unwrap(), json!(word));
    }
    let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
    value["withoutEpic"][0]["priority"] = json!("urgent");
    assert!(serde_json::from_value::<Tickets>(value).is_err());
}

#[test]
fn optional_fields_are_left_out_never_null() {
    let value = serde_json::to_value(sample()).unwrap();
    let in_no_epic = &value["withoutEpic"][0];
    for absent in ["assignee", "priority", "effectivePriority"] {
        assert!(in_no_epic.get(absent).is_none(), "{absent}");
    }
}

#[test]
fn an_answer_of_another_team_is_refused() {
    assert!(sample().check(&id("acme"), &id("site")).is_err());
    assert!(sample().check(&id("other"), &id("app")).is_err());
}

#[test]
fn a_ticket_listed_twice_is_refused() {
    let tickets = changed(|tickets| {
        let child = tickets.epics[0].children[0].clone();
        tickets.without_epic.push(child);
    });
    assert_eq!(check(&tickets), Err("reported app#12 twice".into()));
}

#[test]
fn an_answer_beyond_the_limits_is_refused() {
    let too_many_epics = changed(|tickets| {
        let epic = tickets.epics[0].clone();
        tickets.epics = (0..=MAX_EPICS)
            .map(|n| {
                let mut one = epic.clone();
                one.ticket.reference = format!("app#e{n}");
                one.children = Vec::new();
                one
            })
            .collect();
    });
    assert!(check(&too_many_epics).unwrap_err().contains("epics"));

    let too_many_tickets = changed(|tickets| {
        let ticket = tickets.without_epic[0].clone();
        tickets.without_epic = (0..MAX_TICKETS)
            .map(|n| {
                let mut one = ticket.clone();
                one.reference = format!("app#t{n}");
                one
            })
            .collect();
    });
    assert!(check(&too_many_tickets).unwrap_err().contains("tickets"));

    let at_the_limit = changed(|tickets| {
        let ticket = tickets.without_epic[0].clone();
        tickets.epics.clear();
        tickets.without_epic = (0..MAX_TICKETS)
            .map(|n| {
                let mut one = ticket.clone();
                one.reference = format!("app#t{n}");
                one
            })
            .collect();
    });
    assert_eq!(check(&at_the_limit), Ok(()));
}

#[test]
fn a_ticket_with_text_beyond_the_limits_is_refused() {
    let long = "x".repeat(201);
    let edits: [fn(&mut Tickets, String); 5] = [
        |t, long| t.without_epic[0].reference = long,
        |t, long| t.without_epic[0].title = long,
        |t, long| t.without_epic[0].team = Some(long),
        |t, long| t.epics[0].children[1].assignee = Some(long),
        |t, long| t.epics[0].children[1].pull_requests[0].title = long,
    ];
    for (n, edit) in edits.into_iter().enumerate() {
        let tickets = changed(|tickets| edit(tickets, long.clone()));
        assert!(check(&tickets).is_err(), "edit {n}");
    }
    let empty_ref = changed(|tickets| tickets.without_epic[0].reference.clear());
    assert!(check(&empty_ref).is_err());
    let at_the_limit = changed(|tickets| tickets.without_epic[0].title = "x".repeat(200));
    assert_eq!(check(&at_the_limit), Ok(()));
}

#[test]
fn a_priority_target_follows_an_envelope_targets_rule() {
    for bad in ["", "acme/app#12", "app 12", &"a".repeat(121)] {
        let tickets = changed(|tickets| tickets.without_epic[0].prioritize_with = Some(bad.into()));
        assert!(check(&tickets).is_err(), "{bad:?}");
    }
    let longest = changed(|tickets| {
        tickets.without_epic[0].prioritize_with = Some("a".repeat(120));
    });
    assert_eq!(check(&longest), Ok(()));
}

#[test]
fn pull_requests_are_owner_name_and_a_number_each_listed_once() {
    let pull_request = |repo: &str, number| OpenPullRequest {
        repo: repo.into(),
        number,
        title: "feat: a change".into(),
    };
    for bad in [
        pull_request("app", 1),
        pull_request("acme/app/x", 1),
        pull_request("acme/app", 0),
    ] {
        let tickets = changed(|tickets| tickets.without_epic[0].pull_requests = vec![bad.clone()]);
        assert!(check(&tickets).is_err(), "{bad:?}");
    }
    let twice = changed(|tickets| {
        tickets.without_epic[0].pull_requests =
            vec![pull_request("acme/app", 7), pull_request("acme/app", 7)];
    });
    assert!(check(&twice).unwrap_err().contains("twice"));
    let too_many = changed(|tickets| {
        tickets.without_epic[0].pull_requests = (1..=MAX_PULL_REQUESTS as u64 + 1)
            .map(|n| pull_request("acme/app", n))
            .collect();
    });
    assert!(check(&too_many).unwrap_err().contains("pull requests"));
    let at_the_limit = changed(|tickets| {
        tickets.without_epic[0].pull_requests = (1..=MAX_PULL_REQUESTS as u64)
            .map(|n| pull_request("acme/app", n))
            .collect();
    });
    assert_eq!(check(&at_the_limit), Ok(()));
}

#[test]
fn what_people_wrote_is_served_printable_and_nothing_else_changes() {
    let hidden = "Pay\u{202E}later\u{200B}\nnow";
    let tickets = changed(|tickets| {
        let child = &mut tickets.epics[0].children[1];
        child.title = hidden.into();
        child.team = Some(hidden.into());
        child.assignee = Some(hidden.into());
        child.pull_requests[0].title = hidden.into();
        tickets.without_epic[0].title = hidden.into();
        tickets.epics[0].ticket.title = hidden.into();
    });
    let served = tickets.clone().printable();
    let shown = "Pay?later??now";
    let child = &served.epics[0].children[1];
    assert_eq!(child.title, shown);
    assert_eq!(child.team.as_deref(), Some(shown));
    assert_eq!(child.assignee.as_deref(), Some(shown));
    assert_eq!(child.pull_requests[0].title, shown);
    assert_eq!(served.without_epic[0].title, shown);
    assert_eq!(served.epics[0].ticket.title, shown);
    assert_eq!(sample().printable(), sample(), "plain text stays as it is");
}

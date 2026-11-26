//! A root's node whose root key asks for the owner's presence never signs because someone else asked: it
//! holds joins and device changes, answers `202` with a handle, and the owner signs them in one run after
//! one prompt that names them.
mod common;

use agency_core::devices::{add_device, Added};
use agency_core::invites::{self, finish_joins, join, Joined};
use agency_core::pending_root::{self, sign_held};
use agency_core::views::refresh;
use agency_core::ErrorKind;
use agency_proto::client::DeviceChangeOutcome;
use agency_proto::clock::Duration;
use agency_proto::{AccountKey, Role};
use common::{alice, id, joined_bob, memory_runtime, node, Running};

fn held_ids(alice: &Running) -> Vec<String> {
    let waiting = pending_root::waiting(&alice.node).unwrap();
    waiting.into_iter().map(|one| one.id).collect()
}

async fn held_device_for_bob(alice: &Running, bob: &Running, label: &str) -> AccountKey {
    let key = AccountKey::generate();
    let added = add_device(&bob.node, label, &key.public(), None).await;
    assert!(
        matches!(&added[..], [(_, Added::Done(result))] if result.outcome == DeviceChangeOutcome::Pending
            && result.message.as_deref().is_some_and(|text| text.contains("agency root-sign"))),
        "{added:?}"
    );
    let _ = alice;
    key
}

#[tokio::test]
async fn a_device_change_from_a_member_waits_without_a_prompt_and_one_press_signs_it() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    let before = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version;

    let key = held_device_for_bob(&alice, &bob, "Laptop").await;
    assert!(
        alice.root_keys.prompts().is_empty(),
        "nobody else makes the prompt appear"
    );
    assert_eq!(
        alice
            .node
            .store()
            .load_org(&id("acme"))
            .unwrap()
            .body
            .version,
        before
    );
    let waiting = pending_root::waiting(&alice.node).unwrap();
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].what(), "add device Laptop for bob");

    let run = sign_held(&alice.node, &id("acme"), &held_ids(&alice))
        .await
        .unwrap();
    assert_eq!(run.signed.len(), 1);
    let prompts = alice.root_keys.prompts();
    assert_eq!(prompts.len(), 1);
    assert!(
        prompts[0].contains("add device Laptop for bob"),
        "{prompts:?}"
    );

    refresh(&bob.node).await.unwrap();
    let copy = bob.node.store().load_org(&id("acme")).unwrap();
    assert!(copy
        .body
        .device(&id("bob"), &key.public().device_id())
        .is_some());
    assert!(pending_root::waiting(&alice.node).unwrap().is_empty());
}

#[tokio::test]
async fn changes_are_signed_at_most_three_to_a_prompt_and_every_joiner_takes_its_version() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let before = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version;
    let mut joiners = vec![];
    for name in ["carol", "dave", "erin", "frank"] {
        let joiner = node(name, memory_runtime().0).await;
        let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
        join(&joiner.node, &link, |_| true).await.unwrap();
        joiners.push(joiner);
    }
    let ids = held_ids(&alice);
    let too_many = sign_held(&alice.node, &id("acme"), &ids).await.unwrap_err();
    assert_eq!(too_many.kind, ErrorKind::BadRequest, "four in one prompt");
    for group in ids.chunks(pending_root::MOST_PER_PROMPT) {
        let run = sign_held(&alice.node, &id("acme"), group).await.unwrap();
        assert_eq!(run.signed.len(), group.len(), "{:?}", run.dropped);
    }
    let prompts = alice.root_keys.prompts();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert_eq!(
        prompts[0],
        "Sign 3 changes for acme: add member carol (CAROL), add member dave (DAVE), add member erin (ERIN)"
    );
    let after = alice
        .node
        .store()
        .load_org(&id("acme"))
        .unwrap()
        .body
        .version;
    assert_eq!(after, before + 4);
    for joiner in &joiners {
        let done = finish_joins(&joiner.node).await.unwrap();
        assert!(matches!(&done[..], [Joined::Member(_)]), "{done:?}");
        let copy = joiner.node.store().load_org(&id("acme")).unwrap();
        assert_eq!(
            copy.body.version,
            before
                + 1
                + joiners
                    .iter()
                    .position(|one| std::ptr::eq(one, joiner))
                    .unwrap() as u64
        );
    }
}

#[tokio::test]
async fn a_join_whose_label_could_move_the_terminal_is_refused() {
    use agency_proto::invite::Invite;
    use agency_proto::{JoinRequest, Kind, Signed};
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let invite = Invite::parse(&link).unwrap();
    let key = AccountKey::generate();
    let request = JoinRequest {
        org: id("acme"),
        code: invite.code,
        account: id("mallory"),
        label: "Mallory\n\u{1b}[2Kadd member carol".into(),
        public_key: key.public(),
        at: alice.node.now_text(),
    };
    let signed = Signed::sign(Kind::Join, request, id("mallory"), &key).unwrap();
    let refused = invites::accept_join(&alice.node, &id("acme"), signed)
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::BadRequest);
    assert!(pending_root::waiting(&alice.node).unwrap().is_empty());
}

#[tokio::test]
async fn a_change_that_no_longer_holds_is_dropped_and_the_rest_are_signed() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    // Both are signed with bob's account key, which may add only his first device: once the first is in,
    // the second no longer holds.
    held_device_for_bob(&alice, &bob, "Laptop").await;
    held_device_for_bob(&alice, &bob, "Phone").await;
    let run = sign_held(&alice.node, &id("acme"), &held_ids(&alice))
        .await
        .unwrap();
    assert_eq!(run.signed.len(), 1);
    assert_eq!(run.dropped.len(), 1);
    assert!(pending_root::waiting(&alice.node).unwrap().is_empty());
}

#[tokio::test]
async fn only_the_changes_the_owner_was_shown_are_signed() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;
    let shown = held_ids(&alice);
    held_device_for_bob(&alice, &bob, "Phone").await;
    let run = sign_held(&alice.node, &id("acme"), &shown).await.unwrap();
    assert_eq!(run.signed.len(), 1);
    let left = pending_root::waiting(&alice.node).unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].what(), "add device Phone for bob");
}

#[tokio::test]
async fn a_join_waits_for_the_owner_and_the_joiner_takes_the_version_with_its_handle() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let carol = node("carol", memory_runtime().0).await;
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    let joined = join(&carol.node, &link, |_| true).await.unwrap();
    assert!(matches!(joined, Joined::Waiting(_)), "{joined:?}");
    assert!(alice.root_keys.prompts().is_empty());
    assert!(carol.node.store().load_org(&id("acme")).is_none());

    let dave = node("dave", memory_runtime().0).await;
    assert!(
        join(&dave.node, &link, |_| true).await.is_err(),
        "the invite is held for carol"
    );

    assert!(
        finish_joins(&carol.node).await.unwrap().is_empty(),
        "still waiting"
    );
    sign_held(&alice.node, &id("acme"), &held_ids(&alice))
        .await
        .unwrap();
    let done = finish_joins(&carol.node).await.unwrap();
    assert!(matches!(&done[..], [Joined::Member(_)]), "{done:?}");
    let copy = carol.node.store().load_org(&id("acme")).unwrap();
    assert!(copy.body.member(&id("carol")).is_some());
    assert!(carol.node.store().waiting_joins().is_empty());
}

#[tokio::test]
async fn a_refused_join_frees_its_invite_and_the_joiner_forgets_it() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let carol = node("carol", memory_runtime().0).await;
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let id_of = held_ids(&alice).remove(0);
    pending_root::refuse(&alice.node, &id_of).unwrap();
    finish_joins(&carol.node).await.unwrap();
    assert!(carol.node.store().waiting_joins().is_empty());
    let dave = node("dave", memory_runtime().0).await;
    assert!(matches!(
        join(&dave.node, &link, |_| true).await.unwrap(),
        Joined::Waiting(_)
    ));
}

#[tokio::test]
async fn a_held_change_expires_after_seven_days_and_its_handle_answers_nothing() {
    let (alice, _) = alice().await;
    alice.root_keys.ask_presence(true);
    let carol = node("carol", memory_runtime().0).await;
    let link = invites::invite(&alice.node, &id("acme"), None, Role::Member).unwrap();
    join(&carol.node, &link, |_| true).await.unwrap();
    let handle = held_ids(&alice).remove(0);
    assert!(pending_root::join_answer(&alice.node, &id("acme"), &handle).is_ok());
    let wrong = pending_root::join_answer(&alice.node, &id("acme"), "not-a-handle").unwrap_err();
    assert_eq!(wrong.kind, ErrorKind::NotFound);
    alice.clock.advance(Duration::days(7));
    let gone = pending_root::join_answer(&alice.node, &id("acme"), &handle).unwrap_err();
    assert_eq!(
        gone.kind,
        ErrorKind::NotFound,
        "expired, before anything cleaned up"
    );
    assert!(pending_root::waiting(&alice.node).unwrap().is_empty());
}

#[tokio::test]
async fn a_refused_prompt_signs_nothing_and_keeps_every_change_waiting() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;
    let before = alice.node.store().load_org(&id("acme")).unwrap().digest();
    alice.root_keys.refuse(true);
    let refused = sign_held(&alice.node, &id("acme"), &held_ids(&alice))
        .await
        .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::Forbidden);
    assert_eq!(
        alice.node.store().load_org(&id("acme")).unwrap().digest(),
        before
    );
    assert_eq!(pending_root::waiting(&alice.node).unwrap().len(), 1);
}

#[tokio::test]
async fn the_owners_desk_lists_what_waits_and_no_one_else_sees_it() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;
    let (status, me) = common::body(common::get(&alice, "/api/me").send().await.unwrap()).await;
    assert_eq!(status, 200, "{me}");
    assert_eq!(me["rootChanges"][0]["what"], "add device Laptop for bob");
    assert_eq!(me["rootChanges"][0]["kind"], "addDevice");
    assert_eq!(me["rootChanges"][0]["member"], "bob");
    assert_eq!(me["rootChanges"][0]["label"], "Laptop");
    assert!(me["rootChanges"][0]["device"]
        .as_str()
        .is_some_and(|device| device.starts_with("d-")));
    assert_eq!(me["rootChanges"][0]["org"], "acme");
    let (_, theirs) = common::body(common::get(&bob, "/api/me").send().await.unwrap()).await;
    assert!(theirs.get("rootChanges").is_none(), "{theirs}");
}

#[tokio::test]
async fn one_account_holds_at_most_five_changes_in_an_org() {
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    alice.root_keys.ask_presence(true);
    for label in ["One", "Two", "Three", "Four", "Five"] {
        held_device_for_bob(&alice, &bob, label).await;
    }
    let key = AccountKey::generate();
    let added = add_device(&bob.node, "Six", &key.public(), None).await;
    assert!(
        matches!(&added[..], [(_, Added::Failed(error))] if error.message.contains("already wait")),
        "{added:?}"
    );
    assert_eq!(pending_root::waiting(&alice.node).unwrap().len(), 5);
}

const TOKEN_HEX: &str = "ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12cd34ef56ab12";

async fn register(running: &Running, events: &[&str]) {
    let input = serde_json::json!({ "token": TOKEN_HEX, "environment": "sandbox", "label": "iPhone", "events": events });
    let (status, answer) = common::body(
        reqwest::Client::new()
            .post(format!("{}/api/push/tokens", running.url()))
            .header("x-agency-token", common::TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, 200, "{answer}");
}

fn root_notices(running: &Running) -> Vec<agency_core::notice::NoticeKind> {
    let state = running.notifier.0.lock().unwrap();
    state
        .sent
        .iter()
        .map(|(_, notice)| notice.kind.clone())
        .filter(|kind| matches!(kind, agency_core::notice::NoticeKind::RootChange { .. }))
        .collect()
}

#[tokio::test]
async fn the_owner_is_told_once_of_each_held_change_by_a_token_that_wants_devices() {
    use agency_core::push::notify_waiting;
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    register(&alice, &["device"]).await;
    notify_waiting(&alice.node).await.unwrap();
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;

    notify_waiting(&alice.node).await.unwrap();
    notify_waiting(&alice.node).await.unwrap();
    let told = root_notices(&alice);
    assert_eq!(told.len(), 1, "once: {told:?}");
    let agency_core::notice::NoticeKind::RootChange { what, org, .. } = &told[0] else {
        unreachable!()
    };
    assert_eq!(org, &id("acme"));
    assert_eq!(what.as_deref(), Some("add device Laptop for bob"));
}

#[tokio::test]
async fn hidden_titles_keep_the_labels_out_of_a_held_changes_notice() {
    use agency_core::push::notify_waiting;
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    register(&alice, &["root"]).await;
    alice
        .node
        .update_config(|config| config.hide_titles = true)
        .unwrap();
    notify_waiting(&alice.node).await.unwrap();
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;
    notify_waiting(&alice.node).await.unwrap();
    let told = root_notices(&alice);
    assert!(
        matches!(
            &told[..],
            [agency_core::notice::NoticeKind::RootChange { what: None, .. }]
        ),
        "{told:?}"
    );
}

#[tokio::test]
async fn this_node_writes_no_new_label_that_could_reorder_a_line_but_keeps_old_ones() {
    use agency_proto::org::Department;
    let (alice, _) = alice().await;
    let department = |raw: &str, label: &str| Department {
        id: id(raw),
        label: label.into(),
        parent: None,
        head: None,
    };
    let refused =
        agency_core::departments::add(&alice.node, &id("acme"), department("ops", "Ops\u{202E}x"))
            .unwrap_err();
    assert_eq!(refused.kind, ErrorKind::BadRequest, "{refused:?}");
    agency_core::departments::add(&alice.node, &id("acme"), department("dev", "Dev")).unwrap();
}

#[tokio::test]
async fn a_device_only_token_that_failed_is_sent_a_held_changes_notice_again() {
    use agency_core::push::notify_waiting;
    let (alice, _) = alice().await;
    let bob = joined_bob(&alice).await;
    register(&alice, &["device"]).await;
    notify_waiting(&alice.node).await.unwrap();
    alice.root_keys.ask_presence(true);
    held_device_for_bob(&alice, &bob, "Laptop").await;
    alice.notifier.0.lock().unwrap().refusing = vec![TOKEN_HEX.to_owned()];
    notify_waiting(&alice.node).await.unwrap();
    assert!(root_notices(&alice).is_empty(), "the phone refused it");
    alice.notifier.0.lock().unwrap().refusing = vec![];
    notify_waiting(&alice.node).await.unwrap();
    assert_eq!(root_notices(&alice).len(), 1, "the retry reached it");
}

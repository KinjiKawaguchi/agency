//! The tag of a team's view answer.
use agency_proto::Timestamp;
mod common;
use common::read_vectors::*;
use common::*;

#[test]
fn the_tag_of_a_view_answer_says_nothing_of_when_it_was_made_and_changes_with_everything_else() {
    let answer = sample_answer();
    let tag = answer.tag();
    assert_eq!(tag.len(), 64);
    assert!(
        tag.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "{tag}"
    );

    let mut later = answer.clone();
    later.view.generated_at = Timestamp::parse("2027-01-01T00:00:00Z").unwrap();
    assert_eq!(
        later.tag(),
        tag,
        "the time the runtime made the view is not part of the tag"
    );

    let mut retitled = answer.clone();
    retitled.view.label.push('!');
    assert_ne!(retitled.tag(), tag, "the view");
    let mut other_reader = answer.clone();
    other_reader.viewer = Some("bob".into());
    assert_ne!(other_reader.tag(), tag, "who reads it");
    let mut answering_for = answer;
    answering_for.answers_for = vec![agency_proto::view::AnswersFor {
        name: "finance".into(),
        id: id("carol"),
    }];
    assert_ne!(answering_for.tag(), tag, "whom the reader answers for");
}

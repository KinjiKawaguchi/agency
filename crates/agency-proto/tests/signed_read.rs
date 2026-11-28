//! A signed read, and the id of a node it can be bound to.
use agency_proto::signed::{is_node_id, read_text, read_text_for};
use agency_proto::{AccountKey, Kind};
mod common;
use common::read_vectors::*;
use common::*;

#[test]
fn a_read_bound_to_a_node_is_no_read_for_another_node_and_no_unbound_read() {
    let key = AccountKey::from_seed([7u8; 32]);
    let alice = id("alice");
    let public = key.public();
    let bound = |audience: &str| {
        Kind::ReadFor.message(&read_text_for(
            "GET", READ_PATH, READ_DATE, &alice, audience,
        ))
    };
    let unbound = Kind::Read.message(&read_text("GET", READ_PATH, READ_DATE, &alice));
    let signature = key.sign(&bound(READ_AUDIENCE));
    assert!(public.verify(&bound(READ_AUDIENCE), &signature));
    assert!(
        !public.verify(&bound(OTHER_AUDIENCE), &signature),
        "another node's id"
    );
    assert!(
        !public.verify(&unbound, &signature),
        "the signature is not one over the unbound text"
    );
    let unbound_signature = key.sign(&unbound);
    assert!(public.verify(&unbound, &unbound_signature));
    assert!(
        !public.verify(&bound(READ_AUDIENCE), &unbound_signature),
        "an unbound signature is no bound one"
    );
    let v1_kind_over_bound_lines = Kind::Read.message(&read_text_for(
        "GET",
        READ_PATH,
        READ_DATE,
        &alice,
        READ_AUDIENCE,
    ));
    assert!(
        !public.verify(&v1_kind_over_bound_lines, &signature),
        "the kind line differs, so the bound lines under the v1 kind are not the bound text"
    );
}

#[test]
fn a_node_id_is_thirty_two_lowercase_hex_characters() {
    assert!(is_node_id(READ_AUDIENCE));
    for refused in [
        "",
        "02AE509C6B20999646D3C600687EE1BF",
        "02ae509c6b20999646d3c600687ee1b",
        "02ae509c6b20999646d3c600687ee1bff",
        "02ae509c6b20999646d3c600687ee1bg",
        "02ae509c6b20999646d3c600687ee1b\n",
    ] {
        assert!(!is_node_id(refused), "{refused:?}");
    }
}

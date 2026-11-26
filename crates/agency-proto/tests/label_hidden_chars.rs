//! The characters a label may not hold are written once, in `testdata/label-hidden-chars.json`, and the web client and
//! the Apple clients read the same file. This reads it too, and holds the node's own rules to it exactly: a change to
//! `is_hidden_char`, `label_ok`, `label_ok_to_write`, or `printable` that the file does not say, or a change of the file
//! that the node does not follow, fails here, so no client can print a label differently from the node.
use agency_proto::org::{is_hidden_char, label_ok, label_ok_to_write, printable};
use serde::Deserialize;

#[derive(Deserialize)]
struct Shared {
    mark: String,
    hidden: Vec<(String, String)>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    text: String,
    printable: String,
}

fn shared() -> Shared {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/label-hidden-chars.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn ranges(shared: &Shared) -> Vec<(u32, u32)> {
    shared
        .hidden
        .iter()
        .map(|(low, high)| {
            (
                u32::from_str_radix(low, 16).unwrap(),
                u32::from_str_radix(high, 16).unwrap(),
            )
        })
        .collect()
}

fn is_listed(ranges: &[(u32, u32)], c: char) -> bool {
    ranges
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&u32::from(c)))
}

/// Every character there is, once: the whole of Unicode but the surrogates, which are not characters.
fn every_char() -> impl Iterator<Item = char> {
    (0..=0x10_FFFFu32).filter_map(char::from_u32)
}

#[test]
fn the_mark_and_the_ranges_are_well_formed() {
    let shared = shared();
    assert_eq!(shared.mark, "?");
    let ranges = ranges(&shared);
    assert!(!ranges.is_empty());
    for (low, high) in &ranges {
        assert!(low <= high, "{low:X} to {high:X}");
        assert!(char::from_u32(*low).is_some() && char::from_u32(*high).is_some());
    }
    for pair in ranges.windows(2) {
        assert!(
            pair[0].1 < pair[1].0,
            "sorted, and not overlapping: {pair:?}"
        );
    }
}

#[test]
fn the_nodes_hidden_characters_are_exactly_the_ranges_of_the_file() {
    let shared = shared();
    let ranges = ranges(&shared);
    let different: Vec<String> = every_char()
        .filter(|c| is_hidden_char(*c) != is_listed(&ranges, *c))
        .map(|c| format!("U+{:04X}", u32::from(c)))
        .collect();
    assert_eq!(different, Vec::<String>::new());
}

#[test]
fn a_label_is_written_only_without_a_character_of_the_file_and_printed_with_each_as_the_mark() {
    let shared = shared();
    let ranges = ranges(&shared);
    let mut wrong_write = vec![];
    let mut wrong_print = vec![];
    for c in every_char() {
        let listed = is_listed(&ranges, c);
        let label = format!("a{c}b");
        if label_ok_to_write(&label) == listed {
            wrong_write.push(format!("U+{:04X}", u32::from(c)));
        }
        let expected = if listed {
            shared.mark.clone()
        } else {
            c.to_string()
        };
        if printable(&c.to_string()) != expected {
            wrong_print.push(format!("U+{:04X}", u32::from(c)));
        }
    }
    assert_eq!(wrong_write, Vec::<String>::new(), "label_ok_to_write");
    assert_eq!(wrong_print, Vec::<String>::new(), "printable");
}

#[test]
fn what_an_old_version_may_hold_stays_looser_than_what_is_written_now() {
    // `label_ok` checks every version, old or new, and refuses control characters only; so that nodes old and new
    // agree on which versions are valid, it must not take on the rest of the file.
    let shared = shared();
    let ranges = ranges(&shared);
    for c in every_char() {
        let label = format!("a{c}b");
        assert_eq!(label_ok(&label), !c.is_control(), "U+{:04X}", u32::from(c));
        if c.is_control() {
            assert!(
                is_listed(&ranges, c),
                "a control character is in the file: U+{:04X}",
                u32::from(c)
            );
        }
    }
}

#[test]
fn every_case_of_the_file_is_said_as_the_file_says_it() {
    let shared = shared();
    assert!(shared.cases.len() >= 40, "the file has its examples");
    for case in &shared.cases {
        assert_eq!(printable(&case.text), case.printable, "{:?}", case.text);
        let is_clean = case.printable == case.text;
        let fits = !case.text.is_empty() && case.text.chars().count() <= 80;
        assert_eq!(
            label_ok_to_write(&case.text),
            is_clean && fits,
            "{:?}",
            case.text
        );
    }
}

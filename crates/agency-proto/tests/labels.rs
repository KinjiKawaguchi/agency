//! A label this node writes now can neither move a terminal's cursor nor reorder how a line reads, while every
//! version keeps being checked by the rule all nodes share; a label from someone else is printed with each such
//! character as `?`.
use agency_proto::org::{is_hidden_char, label_ok, label_ok_to_write, printable};

#[test]
fn a_new_label_with_a_control_or_bidirectional_character_is_refused_and_printed_as_a_mark() {
    for hidden in [
        '\n', '\u{1b}', '\u{061C}', '\u{200B}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202E}',
        '\u{2060}', '\u{2066}', '\u{2069}', '\u{FEFF}',
    ] {
        let label = format!("Bob{hidden}cat");
        assert!(!label_ok_to_write(&label), "{hidden:?}");
        assert_eq!(printable(&label), "Bob?cat", "{hidden:?}");
    }
}

#[test]
fn joiners_and_emoji_tags_stay_since_scripts_and_flags_need_them() {
    for label in [
        "می\u{200C}خواهم",
        "क्\u{200D}ष",
        "👩\u{200D}💻 Dev",
        "🏴\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F} Ops",
        "Bob's iPhone 15 (仕事)",
    ] {
        assert!(label_ok_to_write(label), "{label:?}");
        assert_eq!(printable(label), label);
    }
    assert!(!is_hidden_char('\u{200D}'));
}

#[test]
fn a_version_signed_before_keeps_its_label_valid_for_every_node() {
    let old = "Bob\u{202E}cat";
    assert!(label_ok(old), "verification is unchanged");
    assert!(!label_ok_to_write(old));
}

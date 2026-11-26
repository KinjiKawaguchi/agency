//! A label from someone else can neither move a terminal's cursor nor reorder how a line reads: one with a
//! control or a format character is refused, and printed with each such character as `?`.
use agency_proto::org::{label_ok, printable};

#[test]
fn a_label_with_a_control_or_format_character_is_refused_and_printed_as_a_mark() {
    for hidden in [
        '\n',
        '\u{1b}',
        '\u{202E}',
        '\u{2066}',
        '\u{2069}',
        '\u{200B}',
        '\u{200F}',
        '\u{FEFF}',
        '\u{00AD}',
        '\u{E0041}',
    ] {
        let label = format!("Bob{hidden}cat");
        assert!(!label_ok(&label), "{hidden:?}");
        assert_eq!(printable(&label), "Bob?cat", "{hidden:?}");
    }
    assert!(label_ok("Bob's iPhone 15 (仕事)"));
    assert_eq!(printable("Bob's iPhone"), "Bob's iPhone");
}

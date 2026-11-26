//! The lines `agency status` prints about the app releases this node has seen.
use agency_core::clients_seen::{seen, ClientsSeen};
use agency_core::Node;

pub fn seen_lines(node: &Node) -> Vec<String> {
    lines(&seen(node))
}

fn lines(all: &ClientsSeen) -> Vec<String> {
    let mut found = vec![];
    for (platform, versions) in all {
        for (version, one) in versions {
            let build = one
                .last_build
                .map_or_else(String::new, |build| format!(" (build {build})"));
            found.push(format!(
                "  App {platform} {version}{build}, last seen {}",
                one.last_seen
            ));
        }
    }
    if found.is_empty() {
        return found;
    }
    [vec!["Apps seen:".to_owned()], found].concat()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_core::clients_seen::SeenVersion;
    use std::collections::BTreeMap;

    #[test]
    fn what_was_seen_is_listed_by_platform_and_version_and_nothing_when_nothing_was() {
        assert!(lines(&ClientsSeen::new()).is_empty());
        let one = |build: Option<u64>| SeenVersion {
            last_seen: "2026-11-24T12:00:00Z".into(),
            last_build: build,
        };
        let all = BTreeMap::from([
            (
                "ios".to_owned(),
                BTreeMap::from([("0.3.0".to_owned(), one(Some(7)))]),
            ),
            (
                "macos".to_owned(),
                BTreeMap::from([("0.3.0".to_owned(), one(None))]),
            ),
        ]);
        assert_eq!(
            lines(&all),
            [
                "Apps seen:",
                "  App ios 0.3.0 (build 7), last seen 2026-11-24T12:00:00Z",
                "  App macos 0.3.0, last seen 2026-11-24T12:00:00Z",
            ]
        );
    }
}

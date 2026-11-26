//! The lines `agency status` prints about what the org's other nodes speak, as this node last read them.
use agency_core::peer_about::{peers_of, PeerAbout};
use agency_core::Node;
use agency_proto::Id;

pub fn peer_lines(node: &Node, org: &Id) -> Vec<String> {
    peers_of(node, org)
        .iter()
        .map(|(origin, peer)| format!("  Peer {origin}: {}", describe(peer)))
        .collect()
}

fn describe(peer: &PeerAbout) -> String {
    let speaks = match (&peer.about, &peer.read_at) {
        (Some(about), Some(at)) => format!(
            "release {}, federation {:?}, read {at}",
            about.node, about.contracts.federation
        ),
        _ => "not read yet".to_owned(),
    };
    match &peer.error {
        Some(error) => format!("{speaks}; the last try failed: {error}"),
        None => speaks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::about::About;

    fn read() -> PeerAbout {
        PeerAbout {
            about: Some(About::of_release("0.3.0")),
            read_at: Some("2026-11-24T12:00:00Z".into()),
            tried_at: "2026-11-24T12:00:00Z".into(),
            ..PeerAbout::default()
        }
    }

    #[test]
    fn a_peer_is_described_by_what_was_read_and_why_a_try_failed() {
        assert_eq!(
            describe(&read()),
            "release 0.3.0, federation [1], read 2026-11-24T12:00:00Z"
        );
        let failed = PeerAbout {
            error: Some("unreachable".into()),
            ..read()
        };
        assert!(describe(&failed).ends_with("; the last try failed: unreachable"));
        let never = PeerAbout {
            error: Some("unreachable".into()),
            ..PeerAbout::default()
        };
        assert_eq!(
            describe(&never),
            "not read yet; the last try failed: unreachable"
        );
    }
}

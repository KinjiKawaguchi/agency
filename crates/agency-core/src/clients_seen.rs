//! The app releases this node has seen, from the `x-agency-client` header of the requests it admitted
//! (docs/protocol.md, "Client API"): for each platform and version, when it was last seen and the build then.
//! It holds nothing about the person or the device. It gives `oldestApp` in discovery facts to rest on when a
//! breaking Client API change needs a floor.
use crate::error::Result;
use crate::node::Node;
use agency_proto::client_version::ClientVersion;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// At most this many versions are kept for a platform: the ones seen most recently.
pub const MAX_VERSIONS_PER_PLATFORM: usize = 32;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeenVersion {
    /// When a request carrying this version was last admitted (RFC 3339, UTC).
    pub last_seen: String,
    /// The build of that request, when it said one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_build: Option<u64>,
}

/// By platform name, then version.
pub type ClientsSeen = BTreeMap<String, BTreeMap<String, SeenVersion>>;

/// Notes that an admitted request came from `seen`.
pub fn note(node: &Node, seen: &ClientVersion) -> Result<()> {
    let mut all = node.store().clients_seen();
    let versions = all.entry(seen.platform.name().to_owned()).or_default();
    let before = versions.get(&seen.version);
    let last_build = seen.build.or_else(|| before.and_then(|one| one.last_build));
    versions.insert(
        seen.version.clone(),
        SeenVersion {
            last_seen: node.now_text(),
            last_build,
        },
    );
    while versions.len() > MAX_VERSIONS_PER_PLATFORM {
        let oldest = versions
            .iter()
            .min_by(|a, b| a.1.last_seen.cmp(&b.1.last_seen))
            .map(|(version, _)| version.clone());
        match oldest {
            Some(version) => versions.remove(&version),
            None => break,
        };
    }
    node.store().save_clients_seen(&all)
}

/// What the node has seen, by platform and version.
pub fn seen(node: &Node) -> ClientsSeen {
    node.store().clients_seen()
}

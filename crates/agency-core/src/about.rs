//! What this node says about itself: its release, the contracts it speaks, and its capabilities
//! (docs/compatibility.md, "Discovery"). The Client API serves it to apps; the federation route serves it to
//! the members of an org, so a stranger cannot learn a node's version.
use crate::error::Result;
use crate::node::Node;
use crate::read_auth::{verify_read_once, ReadAuth};
use agency_proto::about::{About, Capability};
use agency_proto::client::node_id;
use agency_proto::Id;

/// This release of the node. Every crate of the workspace has the version of the release.
pub const NODE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// What this node says: no floor for apps, which only the Client API's answer may carry.
pub fn this_node() -> About {
    About::of_release(NODE_VERSION)
}

/// What the node at `url` says: `this_node`, without `readAudience` when `url` is no node origin, because other
/// nodes name a node by the id of its origin and could then not make a read that this node takes.
pub fn of_node(url: &str) -> About {
    let mut about = this_node();
    if node_id(url).is_none() {
        about
            .capabilities
            .retain(|name| name != Capability::ReadAudience.name());
    }
    about
}

/// The answer to a member's signed read of `path` (`routes::about`): the same as the Client API's, and
/// nothing else, so it does not depend on who asks once they are a member.
pub fn read_about(node: &Node, org: &Id, auth: &ReadAuth, path: &str) -> Result<About> {
    let doc = node.store().load_org(org);
    let definition = doc.as_ref().map(|doc| &doc.body);
    verify_read_once(node, definition, doc.is_some(), auth, "GET", path)?;
    Ok(of_node(&node.url()))
}

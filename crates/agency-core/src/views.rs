//! TeamViews: served for the teams this node hosts, and kept as replicas for teams on other nodes.
mod addressing;
mod refresh;
mod replica;
mod served;

pub use addressing::{
    addressed_party, answerer_view, answerers, is_addressed_to, mine, remote_addressed_party,
    viewer_name, waiting_on_viewer, Reader,
};
pub use refresh::{refresh, refresh_now, waiting_lines, READS_AT_ONCE};
pub use replica::Replica;
pub(crate) use served::runtime_view;
pub use served::{check_view, hosted_view};

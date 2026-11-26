//! What a push notification is about, and what it may say (docs/protocol.md, "Push notifications").
//! A title or label is `None` when the node leaves it out (`push.hideTitles`) or when there is none;
//! the adapter then sends the wording that names no title.
use agency_proto::client::PushEvent;
use agency_proto::Id;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// A request that waits for the owner.
    Request {
        org: Id,
        team: Id,
        request: String,
        team_label: Option<String>,
        title: Option<String>,
    },
    /// A device that asks to be added: its id (`d-` and its pairing code) and the label it gave.
    Device { device: Id, label: Option<String> },
    /// A decision of the owner's that the team's node refused: the decision's id, and the request's title
    /// the decision log kept.
    Decision {
        org: Id,
        team: Id,
        id: String,
        title: Option<String>,
    },
}

/// One notification, and the badge: how many requests are the owner's to decide across all their teams
/// when it is sent (a request's own notification included).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub badge: u32,
}

impl Notice {
    pub fn event(&self) -> PushEvent {
        match self.kind {
            NoticeKind::Request { .. } => PushEvent::Request,
            NoticeKind::Device { .. } => PushEvent::Device,
            NoticeKind::Decision { .. } => PushEvent::Decision,
        }
    }

    /// Notifications of one team are grouped together; the devices have a group of their own.
    pub fn thread(&self) -> String {
        match &self.kind {
            NoticeKind::Request { org, team, .. } | NoticeKind::Decision { org, team, .. } => {
                format!("{org}/{team}")
            }
            NoticeKind::Device { .. } => "devices".to_owned(),
        }
    }

    /// What replaces an earlier notification about the same thing: the request, the device, or the
    /// decision.
    pub fn collapse_id(&self) -> &str {
        match &self.kind {
            NoticeKind::Request { request, .. } => request,
            NoticeKind::Device { device, .. } => device.as_str(),
            NoticeKind::Decision { id, .. } => id,
        }
    }
}

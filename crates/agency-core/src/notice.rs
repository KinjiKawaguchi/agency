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
    /// A recovery of an org whose root is the owner's account was started, and when it takes effect on
    /// this node. It names the org by id, never by label, so `hideTitles` keeps it whole.
    Recovery {
        org: Id,
        takes_effect_at: Option<String>,
    },
    /// A change someone asked the root to sign waits for the owner: its handle, and what it changes (absent
    /// when the node hides titles, since it names a member's or a device's label).
    RootChange {
        org: Id,
        id: String,
        what: Option<String>,
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
            NoticeKind::Recovery { .. } => PushEvent::Recovery,
            NoticeKind::RootChange { .. } => PushEvent::Root,
        }
    }

    /// Whether a token or a browser gets it whatever kinds it names.
    pub fn is_for_everyone(&self) -> bool {
        matches!(self.kind, NoticeKind::Recovery { .. })
    }

    /// Whether a token or a browser that names `events` gets it.
    pub fn is_wanted_by(&self, events: &[PushEvent]) -> bool {
        let is_device_like = self.event() == PushEvent::Root && events.contains(&PushEvent::Device);
        self.is_for_everyone() || events.contains(&self.event()) || is_device_like
    }

    /// Notifications of one team are grouped together; the devices have a group of their own.
    pub fn thread(&self) -> String {
        match &self.kind {
            NoticeKind::Request { org, team, .. } | NoticeKind::Decision { org, team, .. } => {
                format!("{org}/{team}")
            }
            NoticeKind::Device { .. } => "devices".to_owned(),
            NoticeKind::Recovery { org, .. } => format!("recovery/{org}"),
            NoticeKind::RootChange { org, .. } => format!("root/{org}"),
        }
    }

    /// What replaces an earlier notification about the same thing: the request, the device, or the
    /// decision.
    pub fn collapse_id(&self) -> &str {
        match &self.kind {
            NoticeKind::Request { request, .. } => request,
            NoticeKind::Device { device, .. } => device.as_str(),
            NoticeKind::Decision { id, .. } => id,
            NoticeKind::Recovery { org, .. } => org.as_str(),
            NoticeKind::RootChange { id, .. } => id,
        }
    }
}

/// The words of a recovery notice, written by the node so that an app or a browser worker that does not know
/// the kind yet still shows it: a title naming the org's id, and when it takes effect.
pub fn recovery_text(org: &Id, takes_effect_at: Option<&str>) -> (String, String) {
    let title = format!("A recovery of {org} was started");
    let body = match takes_effect_at {
        Some(at) => format!(
            "It takes effect at {at}. If you did not start it, sign any version of the org now, for example with agency root-cancel-recovery."
        ),
        None => "If you did not start it, sign any version of the org now, for example with agency root-cancel-recovery.".to_owned(),
    };
    (title, body)
}

/// The words of a held root change's notice, written by the node as a recovery's are: what waits, and where
/// to sign it. Without `what` (hidden titles), it names only the org.
pub fn root_change_text(org: &Id, what: Option<&str>) -> (String, String) {
    let title = format!("A change to {org} waits for the root key");
    let body = match what {
        Some(what) => format!("{what}. Run agency root-sign on the root key's node to sign it."),
        None => "Run agency root-sign on the root key's node to sign it.".to_owned(),
    };
    (title, body)
}

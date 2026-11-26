//! The Client API between a client (web, iOS, desktop) and its home node (`/api`). See "Client API" in
//! docs/protocol.md. Every `Option` is omitted from JSON when absent.
use crate::envelope::{Envelope, EnvelopeKind};
use crate::ids::Id;
use crate::keys::PublicKey;
use crate::org::Device;
use crate::signed::Signed;
use crate::timestamp::Timestamp;
use crate::view::TeamView;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// What a client needs before it writes: the write token, and the node's clock, which an envelope's `at`
/// must agree with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Session {
    /// The value every write sends in `x-agency-token`. It changes each time the node starts.
    pub write_token: String,
    /// The node's time when it answered.
    pub now: Timestamp,
}

/// The node's account and everything it can reach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Me {
    pub account: AccountRef,
    pub orgs: Vec<OrgRef>,
    pub funding: Vec<Funding>,
    /// Whether the node keeps titles and labels off every notification (`hideTitles`): a client that shows
    /// them outside the app, such as a widget, leaves them out too. Left out when the node does not.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(optional, type = "boolean")]
    pub hides_titles: bool,
    /// This node's id as it names itself in a push (`agency.node`, `node_id` of its `url`), so that an app
    /// maps a push to the node it talks to without working the id out from the address it signed in with,
    /// which may not be the node's own `url`. Left out when the node's `url` is not a home node's origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub node_id: Option<String>,
    /// The other home nodes of the same human, for their other accounts, as the node's owner
    /// configured them. Given only to the owner, and absent when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub home_nodes: Option<Vec<HomeNode>>,
}

/// Another home node of the same human: the home node of another of their accounts (for example a work
/// account). It says only where that node is; nothing of it is copied, and a client signs in to it and
/// keeps a device key for it on its own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HomeNode {
    pub label: String,
    /// Its node origin: `https://<host>[:<port>]`, or `http://` on this machine's loopback, with no path.
    pub url: String,
}

/// A home node's origin as a client uses it, written one way, or `None` when `url` cannot be one:
/// `https://<host>[:<port>]`, or `http://` only on the loopback (`localhost`, `127.0.0.1`, `[::1]`), with
/// no user name, path, query, or fragment (a trailing `/` is dropped). The host is dot-joined labels of
/// letters, digits, and hyphens, lowercased; `[::1]` is the only address in brackets. The port is 1 to
/// 65535, and the scheme's default port is left out, so `https://Work.Example.com:443/` is
/// `https://work.example.com`.
pub fn home_node_origin(url: &str) -> Option<String> {
    let url = url.strip_suffix('/').unwrap_or(url);
    let (scheme, rest, default_port) = match url.split_once("://")? {
        ("https", rest) => ("https", rest, 443),
        ("http", rest) => ("http", rest, 80),
        _ => return None,
    };
    let (host, port) = match rest.strip_prefix("[::1]") {
        Some(after) => ("[::1]".to_owned(), after),
        None => {
            let colon = rest.find(':').unwrap_or(rest.len());
            (rest[..colon].to_ascii_lowercase(), &rest[colon..])
        }
    };
    let port = match port {
        "" => None,
        with_colon => {
            let digits = with_colon.strip_prefix(':')?;
            let is_digits = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
            let number = digits
                .parse::<u16>()
                .ok()
                .filter(|n| is_digits && *n >= 1)?;
            (number != default_port).then_some(number)
        }
    };
    let is_label = |label: &str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let is_name = host != "[::1]" && host.len() <= 253 && host.split('.').all(is_label);
    let is_loopback = matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]");
    let is_allowed = match scheme {
        "https" => is_name,
        _ => is_loopback,
    };
    is_allowed.then(|| match port {
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    })
}

/// The id a node puts in its pushes (`agency.node`), so that an app with several home nodes knows which one sent it without
/// the push naming the host to Apple: the first 16 bytes of the SHA-256 of `agency-node:` and the node's origin
/// (`home_node_origin`), as 32 lowercase hex characters. `None` when `url` is not a home node's origin. The apps work out the
/// same id for each node they know (`testdata/node-id.json`).
pub fn node_id(url: &str) -> Option<String> {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let origin = home_node_origin(url)?;
    let digest = Sha256::digest(format!("agency-node:{origin}").as_bytes());
    Some(
        digest[..16]
            .iter()
            .fold(String::with_capacity(32), |mut id, byte| {
                let _ = write!(id, "{byte:02x}");
                id
            }),
    )
}

/// Whether `url` can be a home node's origin for a client (`home_node_origin`).
pub fn is_home_node_origin(url: &str) -> bool {
    home_node_origin(url).is_some()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AccountRef {
    pub id: Id,
    pub label: String,
    /// The account key's fingerprint.
    pub fingerprint: String,
}

/// An org the account belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgRef {
    pub id: Id,
    pub label: String,
    /// Whether this node holds the org's root key.
    pub is_root: bool,
    pub teams: Vec<TeamRef>,
    /// A recovery of the org that was started and waits. Left out when none waits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub recovery: Option<StartedRecovery>,
}

/// A recovery of an org that was started with its recovery key and waits (docs/protocol.md, "Recovery
/// key"). Until it takes effect, the root cancels it by signing any version of the org.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct StartedRecovery {
    /// When it takes effect on this node (`RECOVERY_WAIT_SECONDS` after this node took the start). Absent
    /// when this node keeps no record of when it took it, so it does not take the finish either.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub takes_effect_at: Option<String>,
    /// The fingerprint of the root key it moves the org to; absent when it keeps the root key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_root_key: Option<String>,
    /// Whether it replaces the recovery key.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    #[ts(type = "boolean")]
    pub replaces_recovery_key: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct TeamRef {
    pub id: Id,
    pub label: String,
    pub location: TeamLocation,
    /// The viewer's name in the team runtime. Absent when the team does not address the viewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TeamLocation {
    /// The team runs on this node.
    Local,
    /// The team runs on another node; this node keeps a replica.
    Remote,
}

/// A team's view as the client needs it: the view, who the viewer is in it, and how fresh it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct TeamSnapshot {
    pub view: TeamView,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer: Option<String>,
    /// The ids of the requests that are the viewer's to decide, by the rule in "Requests"
    /// (docs/protocol.md). Clients use this instead of working it out from `to`.
    pub mine: Vec<String>,
    /// The `id`s of the team's people whose `waitingOn` is the viewer (see "People" in docs/protocol.md).
    pub waiting_on_viewer: Vec<String>,
    pub source: SnapshotSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SnapshotSource {
    /// Read from the team runtime just now.
    Local,
    /// The last replica this node read from the team's node. `error` is the last failed read, if the
    /// most recent read failed; the view is then older than `triedAt`.
    #[serde(rename_all = "camelCase")]
    Replica {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        fetched_at: Option<Timestamp>,
        tried_at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        error: Option<String>,
    },
}

/// The longest `title` and `choiceLabel`, in characters.
pub const MAX_NOTE: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DecisionResult {
    pub outcome: DecisionOutcome,
    /// Why a decision was queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DecisionOutcome {
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node, which applied it.
    Delivered,
    /// The team's node could not be reached; the signed decision waits in this node's outbox.
    Queued,
}

/// A source of funding of the account, such as a subscription.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct Funding {
    pub id: Id,
    pub label: String,
    pub kind: FundingKind,
    /// Usage windows that could be read. Empty when the usage is unknown.
    pub windows: Vec<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_at: Option<Timestamp>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FundingKind {
    Subscription,
    ApiKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UsageWindow {
    pub name: UsageWindowName,
    /// The window's length: 5 for `fiveHour`, 168 for `sevenDay`.
    pub length_hours: u32,
    /// Percent of the window used, rounded to an integer.
    pub utilization: u32,
    pub resets_at: Timestamp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum UsageWindowName {
    FiveHour,
    SevenDay,
}

impl UsageWindowName {
    pub fn length_hours(self) -> u32 {
        match self {
            Self::FiveHour => 5,
            Self::SevenDay => 7 * 24,
        }
    }
}

/// The decisions the node's account made recently, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DecisionLog {
    pub entries: Vec<DecisionEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DecisionEntry {
    /// The decision envelope's id.
    pub id: String,
    pub org: Id,
    pub team: Id,
    pub kind: EnvelopeKind,
    /// The request id or work item's item reference it answered.
    pub target: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_label: Option<String>,
    pub decided_at: Timestamp,
    pub state: DecisionState,
}

/// Where a decision went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DecisionState {
    /// Applied to a team on this node.
    Applied,
    /// Delivered to the team's node, which applied it.
    Delivered,
    /// Waiting in the outbox; the team's node has not been reached yet.
    #[serde(rename_all = "camelCase")]
    Queued {
        attempts: u32,
        next_at: Timestamp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        last_error: Option<String>,
    },
    /// The team's node refused it; it will not be retried.
    Refused { message: String },
}

/// A decision signed by one of the account's devices, with notes for the decision log.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct SignedDecisionInput {
    pub envelope: Signed<Envelope>,
    /// The request's title, kept in the decision log for people to read (at most 200 characters).
    /// Never signed or sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The label of the chosen option, kept in the decision log like `title`. Only with a choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice_label: Option<String>,
}

/// A new device asking to be added. One of the account's devices (or, for the first device, the node's
/// terminal) adds it after the person compares the pairing codes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DeviceRequestInput {
    pub label: String,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DeviceRequest {
    pub label: String,
    pub public_key: PublicKey,
    /// The id the device gets when it is added: `d-` and the pairing code without dashes.
    pub device_id: Id,
    pub pairing_code: String,
    pub requested_at: Timestamp,
}

/// The account's devices in each org it belongs to, and the devices waiting to be added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DeviceList {
    pub orgs: Vec<OrgDevices>,
    pub requests: Vec<DeviceRequest>,
    /// Where and how to approve a device on this node's terminal. Only the node's owner is given it
    /// (the command holds the path of the running program, which can hold a user name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeInfo>,
}

/// What the owner needs to approve a device at this node's terminal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NodeInfo {
    /// The node's origin (`url` of its configuration).
    pub url: String,
    /// The absolute data directory the running node uses.
    pub data_dir: String,
    /// What to run on the node's machine to approve a device: the absolute path of the running
    /// program, `--dir` and the data directory unless it is `~/.agency`, and `device-add`, with a path
    /// that needs it quoted for a POSIX shell.
    pub command: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OrgDevices {
    pub org: Id,
    pub devices: Vec<Device>,
}

/// Where a device change went.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, optional_fields)]
pub struct DeviceChangeResult {
    pub outcome: DeviceChangeOutcome,
    /// Why a change was queued.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DeviceChangeOutcome {
    /// This node holds the org's root key and wrote the next version.
    Applied,
    /// The root's node wrote the next version; this node has it.
    Delivered,
    /// The root's node could not be reached; the change waits in the outbox.
    Queued,
}

/// Which APNs host a device token belongs to: `sandbox` for development builds, `production` for
/// TestFlight and App Store builds. The app says which; the node never guesses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PushEnvironment {
    Production,
    Sandbox,
}

/// What a push notification is about: a request that waits for the owner, a device that asks to be
/// added, or a decision of the owner's that the team's node refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PushEvent {
    Request,
    Device,
    Decision,
    /// A recovery of an org whose root is this node's account was started. Every token and browser gets
    /// it, whatever kinds it names: the owner must be able to cancel.
    Recovery,
}

/// What a token gets when it names no kinds (what a token got before kinds existed).
pub fn default_push_events() -> Vec<PushEvent> {
    vec![PushEvent::Request]
}

/// An APNs device token to register for the owner's notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct PushTokenInput {
    /// The device token as hex.
    pub token: String,
    pub environment: PushEnvironment,
    pub label: String,
    /// The kinds of notification this token wants. Absent means `request` only; an empty list means
    /// none. Repeated kinds count once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<PushEvent>>,
}

/// A registered device token. In a list, `token` holds only its last 8 characters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushToken {
    pub token: String,
    pub environment: PushEnvironment,
    pub label: String,
    pub registered_at: Timestamp,
    /// The kinds of notification it gets, in the order `request`, `device`, `decision`. A token stored
    /// before kinds existed reads as `request` only.
    #[serde(default = "default_push_events")]
    pub events: Vec<PushEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushTokenList {
    pub tokens: Vec<PushToken>,
}

/// The node's VAPID public key, which a browser subscribes with (`applicationServerKey`), and a short
/// fingerprint of it. A subscription made with another key is void.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VapidKey {
    /// Uncompressed P-256 point, base64url without padding.
    pub public_key: String,
    pub key_id: String,
}

/// A browser's Web Push subscription to register for the owner's notifications.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct PushSubscriptionInput {
    /// The push service's URL for this browser: `https`, on a host the node allows, at most 500 characters.
    pub endpoint: String,
    /// The browser's P-256 public key (65 octets, base64url without padding).
    pub p256dh: String,
    /// The browser's authentication secret (16 octets, base64url without padding).
    pub auth: String,
    pub label: String,
    /// The kinds of notification this subscription wants; as for `PushTokenInput`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<PushEvent>>,
}

/// A registered Web Push subscription. The keys are never given back, and `endpoint` is shortened to its
/// host and the last 8 characters of its path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushSubscription {
    /// A hash of the endpoint: what a DELETE names.
    pub id: String,
    pub endpoint: String,
    pub label: String,
    pub registered_at: Timestamp,
    pub events: Vec<PushEvent>,
    /// The `keyId` of the node's VAPID key it was made with.
    pub key_id: String,
    /// When the push service last took a notice for this browser; left out until it has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_succeeded_at: Option<Timestamp>,
    /// The last time the push service did not take one, and why (the host and the status it answered, or that it
    /// could not be reached; never what it said). Left out until there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_failure: Option<PushFailure>,
}

/// A notice the push service did not take: when, and the reason the node can give without the service's words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushFailure {
    pub at: Timestamp,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PushSubscriptionList {
    pub subscriptions: Vec<PushSubscription>,
}

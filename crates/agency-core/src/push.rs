//! Push notifications for the owner (docs/protocol.md, "Push notifications"). The owner's app registers
//! its APNs device tokens; the node notifies each of them once when a new request waits for the owner.
//! Only the team label and the request title leave the node.
use crate::error::{Error, Result};
use crate::node::Node;
use crate::views::runtime_view;
use agency_proto::client::{PushToken, PushTokenInput, PushTokenList};
use agency_proto::view::Request;
use agency_proto::{Id, OrgDefinition, Timestamp};

/// The most tokens kept; the oldest is dropped first.
pub const MAX_TOKENS: usize = 20;
/// How many notified requests are remembered.
pub const NOTIFIED_KEPT: usize = 1000;

/// One notification: what the person sees, and where tapping it leads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// The team's label.
    pub title: String,
    /// The request's title.
    pub body: String,
    pub org: Id,
    pub team: Id,
    pub request: String,
}

impl Notice {
    /// Notifications of one team are grouped together.
    pub fn thread(&self) -> String {
        format!("{}/{}", self.org, self.team)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushError {
    /// The token is no longer valid; it is removed.
    Gone,
    /// Sending failed now; the request is tried again on the next pass.
    Failed(String),
}

/// A device token as hex, lowercased: an even number of 32 to 200 hex characters.
fn check_token(raw: &str) -> Result<String> {
    let token = raw.trim().to_ascii_lowercase();
    let is_hex = token.bytes().all(|b| b.is_ascii_hexdigit());
    if !is_hex || !(32..=200).contains(&token.len()) || !token.len().is_multiple_of(2) {
        return Err(Error::bad_request(
            "the token must be hex, 32 to 200 characters",
        ));
    }
    Ok(token)
}

/// Registers a device token, or updates its environment and label.
pub fn register_push_token(node: &Node, input: PushTokenInput) -> Result<PushToken> {
    let token = check_token(&input.token)?;
    let label = input.label.trim().to_owned();
    if !(1..=80).contains(&label.chars().count()) || label.chars().any(char::is_control) {
        return Err(Error::bad_request("the label is 1 to 80 characters"));
    }
    let registered_at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let entry = PushToken {
        token,
        environment: input.environment,
        label,
        registered_at,
    };
    let mut tokens: Vec<PushToken> = node
        .store()
        .push_tokens()
        .into_iter()
        .filter(|one| one.token != entry.token)
        .collect();
    tokens.push(entry.clone());
    let start = tokens.len().saturating_sub(MAX_TOKENS);
    node.store().save_push_tokens(&tokens[start..])?;
    Ok(entry)
}

/// Removes a device token. Removing an unknown token is not an error.
pub fn remove_push_token(node: &Node, raw: &str) -> Result<()> {
    let token = check_token(raw)?;
    remove_tokens(node, &[token])
}

fn remove_tokens(node: &Node, gone: &[String]) -> Result<()> {
    let kept: Vec<PushToken> = node
        .store()
        .push_tokens()
        .into_iter()
        .filter(|one| !gone.contains(&one.token))
        .collect();
    node.store().save_push_tokens(&kept)
}

/// The registered tokens, each shortened to its last 8 characters.
pub fn push_tokens(node: &Node) -> PushTokenList {
    let shorten = |one: PushToken| PushToken {
        token: one.token[one.token.len().saturating_sub(8)..].to_owned(),
        ..one
    };
    PushTokenList {
        tokens: node
            .store()
            .push_tokens()
            .into_iter()
            .map(shorten)
            .collect(),
    }
}

/// A request waiting for the owner, and its key in the notified list.
struct Waiting {
    key: String,
    notice: Notice,
}

fn waiting(org: &Id, team: &Id, label: &str, request: &Request) -> Waiting {
    Waiting {
        key: format!("{org}/{team}/{}", request.id),
        notice: Notice {
            title: label.to_owned(),
            body: request.title.clone(),
            org: org.clone(),
            team: team.clone(),
            request: request.id.clone(),
        },
    }
}

/// Requests addressed to the owner in one team: through the hosted-team configuration for a team on
/// this node, and through the owner's runtime name (`viewer`) for a replica.
async fn waiting_in(node: &Node, definition: &OrgDefinition, team: &Id) -> Vec<Waiting> {
    let org = &definition.org;
    let Some(person) = definition.member(&node.account()) else {
        return vec![];
    };
    if let Some(hosted) = node.hosted(org, team) {
        let Ok(view) = runtime_view(node, &hosted).await else {
            return vec![];
        };
        let mine = view
            .requests
            .iter()
            .filter(|request| hosted.addressee(request.to.as_deref()) == &person.id);
        return mine
            .map(|request| waiting(org, team, &view.label, request))
            .collect();
    }
    let replica = node.store().load_replica(org, team).unwrap_or_default();
    let (Some(view), Some(viewer)) = (replica.view, replica.viewer) else {
        return vec![];
    };
    let is_mine = |request: &&Request| match request.to.as_deref() {
        Some(to) => to == viewer,
        None => viewer == "owner",
    };
    let mine = view.requests.iter().filter(is_mine);
    mine.map(|request| waiting(org, team, &view.label, request))
        .collect()
}

async fn waiting_for_owner(node: &Node) -> Vec<Waiting> {
    let mut all = vec![];
    for doc in node.store().orgs() {
        for place in &doc.body.teams {
            all.extend(waiting_in(node, &doc.body, &place.id).await);
        }
    }
    all
}

/// What one pass sent, and the failures to report.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pass {
    pub sent: usize,
    pub failures: Vec<String>,
}

/// Sends one notice to every token. It counts as delivered if any token took it, or if no token is
/// registered; otherwise the request is tried again on the next pass. Tokens APNs no longer accepts are
/// removed.
async fn send_to_all(node: &Node, notice: &Notice, pass: &mut Pass) -> Result<bool> {
    let tokens = node.store().push_tokens();
    let mut delivered = tokens.is_empty();
    let mut gone = vec![];
    for token in &tokens {
        match node.notifier().send(token, notice).await {
            Ok(()) => delivered = true,
            Err(PushError::Gone) => gone.push(token.token.clone()),
            Err(PushError::Failed(why)) => pass.failures.push(why),
        }
    }
    if !gone.is_empty() {
        remove_tokens(node, &gone)?;
        delivered |= gone.len() == tokens.len();
    }
    Ok(delivered)
}

/// Notifies the owner of each request that started waiting since the last pass. The first pass only
/// records the requests already waiting, so nothing old is sent.
pub async fn notify_waiting(node: &Node) -> Result<Pass> {
    let waiting = waiting_for_owner(node).await;
    let Some(mut notified) = node.store().notified() else {
        let keys: Vec<String> = waiting.into_iter().map(|one| one.key).collect();
        node.store().save_notified(&keys)?;
        return Ok(Pass::default());
    };
    let fresh: Vec<Waiting> = waiting
        .into_iter()
        .filter(|one| !notified.contains(&one.key))
        .collect();
    let mut pass = Pass::default();
    for one in fresh {
        if send_to_all(node, &one.notice, &mut pass).await? {
            pass.sent += 1;
            notified.push(one.key);
        }
    }
    let start = notified.len().saturating_sub(NOTIFIED_KEPT);
    node.store().save_notified(&notified[start..])?;
    Ok(pass)
}

//! Invites and joins (kind `join`). The root key's node creates a single-use invite that expires after
//! seven days; the invited node checks the root key's fingerprint, then sends a signed join request.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{is_root, load, publish, sign};
use crate::pending_root::{hold, must_hold, PendingRootChange, RootChange, WaitingJoin};
use crate::ports::DefinitionAccess;
use crate::push_devices::keep_org;
use crate::read_auth::sign_for_peer;
use crate::routes;
use agency_proto::clock::{self, Duration};
use agency_proto::invite::{code_digest, code_from_bytes, Invite};
use agency_proto::org::{verify_root_successor, SignedOrg};
use agency_proto::root_change::RootAnswer;
use agency_proto::runtime::is_reserved_name;
use agency_proto::seat::Seat;
use agency_proto::{Id, JoinRequest, Kind, OrgDefinition, Person, Role, Signed};
use serde::{Deserialize, Serialize};

pub const INVITE_DAYS: i64 = 7;
/// How far a join request's time may be from the root node's clock, either way.
pub const JOIN_SKEW_SECONDS: i64 = 600;

/// What an invite grants. Stored under the SHA-256 of its code; the code itself is never stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InviteRecord {
    pub org: Id,
    pub department: Option<Id>,
    pub role: Role,
    pub expires_at: String,
    pub used_by: Option<Id>,
    /// The seat whoever joins with it takes (docs/glossary.md, "Claim link"); the department is the seat's.
    #[serde(default)]
    pub seat: Option<Id>,
}

/// On the root key's node: creates an invite and returns its link.
pub fn invite(node: &Node, org: &Id, department: Option<Id>, role: Role) -> Result<String> {
    create_invite(node, org, department, None, role)
}

/// On the root key's node: creates an invite for a seat (a claim link). Whoever joins with it takes the seat,
/// and with it the seat's department. It names a seat of the org now; it is checked again when someone joins.
pub fn invite_for_seat(node: &Node, org: &Id, seat: Id, role: Role) -> Result<String> {
    create_invite(node, org, None, Some(seat), role)
}

fn create_invite(
    node: &Node,
    org: &Id,
    department: Option<Id>,
    seat: Option<Id>,
    role: Role,
) -> Result<String> {
    let current = load(node, org)?;
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("only the node with the root key of org {org} can invite"),
        ));
    }
    if let Some(seat) = &seat {
        if !current.body.seats.iter().any(|one| &one.id == seat) {
            return Err(Error::not_found(format!("no seat {seat} in org {org}")));
        }
    }
    let code = code_from_bytes(node.random());
    let expires_at = clock::format(node.now() + Duration::days(INVITE_DAYS));
    let record = InviteRecord {
        org: org.clone(),
        department,
        role,
        expires_at,
        used_by: None,
        seat,
    };
    node.store().save_invite(&code_digest(&code), &record)?;
    Ok(Invite {
        node: node.url(),
        org: org.clone(),
        fp: current.body.root.public_key.fingerprint(),
        code,
    }
    .to_link())
}

/// The invite, if it is for this org, unused, and not expired.
pub fn open_invite(node: &Node, org: &Id, code: &str) -> Option<InviteRecord> {
    let record = node.store().load_invite(&code_digest(code))?;
    let is_open = &record.org == org
        && record.used_by.is_none()
        && clock::parse(&record.expires_at).is_some_and(|at| at > node.now());
    is_open.then_some(record)
}

/// On the root key's node: checks the join request's signature, the invite, and duplicates, then signs
/// the next version with the new person added and returns it.
pub async fn accept_join(node: &Node, org: &Id, signed: Signed<JoinRequest>) -> Result<RootAnswer> {
    let request = &signed.body;
    // Anyone can send a join request, and a request is signed by the key it adds, so a signature proves
    // nothing yet: the checks that need no state, and the invite, come before the write lock, which only a
    // request with an open invite waits for. The invite is read again under the lock.
    if &request.org != org
        || signed.signer != request.account
        || !signed.verify(Kind::Join, &request.public_key)
    {
        return Err(Error::unauthorized(
            "the join request's signature does not match",
        ));
    }
    let is_label_valid = agency_proto::org::label_ok_to_write(&request.label);
    let is_fresh = clock::parse(&request.at)
        .is_some_and(|at| (node.now() - at).whole_seconds().abs() <= JOIN_SKEW_SECONDS);
    if !is_label_valid || !is_fresh {
        return Err(Error::bad_request("malformed name or time"));
    }
    // Refused here as well as when the version is signed, so that such a join is never held for the root.
    if is_reserved_name(&request.account) {
        return Err(Error::bad_request(format!(
            "{} is a name a team runtime reserves; join with another account id",
            request.account
        )));
    }
    let no_invite = || {
        Error::new(
            ErrorKind::Forbidden,
            "no such invite, or it expired or was used",
        )
    };
    open_invite(node, org, &request.code).ok_or_else(no_invite)?;
    let _guard = node.write_lock().lock().await;
    let record = open_invite(node, org, &request.code).ok_or_else(no_invite)?;
    let current = load(node, org)?;
    if current
        .body
        .people
        .iter()
        .any(|one| one.account == request.account || one.id == request.account)
    {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that account is already in the org",
        ));
    }
    // Built before the invite is used, so that a seat that is gone leaves the invite as it was.
    let body = joined_body(node, &current, request, &record)?;
    let use_invite = || {
        node.store().save_invite(
            &code_digest(&request.code),
            &InviteRecord {
                used_by: Some(request.account.clone()),
                ..record.clone()
            },
        )
    };
    // A held join uses the invite at once: it waits for the owner, and no other join may take it meanwhile.
    // A join the node does not hold (too many wait already) gives the invite back.
    if must_hold(node, &current) {
        use_invite()?;
        let held = Box::new(signed.clone());
        return hold(node, org, RootChange::Join { request: held }).or_else(|refused| {
            node.store()
                .save_invite(&code_digest(&request.code), &record)?;
            Err(refused)
        });
    }
    // Otherwise the invite is used only once the root has signed, so a join refused at signing keeps it. The
    // write lock is held throughout, so no other join with the same invite comes in between.
    let doc = sign(node, body)?;
    use_invite()?;
    publish(node, &doc)?;
    Ok(RootAnswer::Signed(Box::new(doc)))
}

/// The next version with the joiner added, as the invite says; refused when the account is a member by now.
/// For a seat, the seat is gone from `seats` and the joiner has its id in `formerly`, its department, and
/// what to ask about it; refused when the seat is not in the org any more. The fields are `orgSeats`, which
/// every node of the org lists already, since the org has the seat.
pub(crate) fn joined_body(
    node: &Node,
    current: &SignedOrg,
    request: &JoinRequest,
    record: &InviteRecord,
) -> Result<OrgDefinition> {
    let is_in = current
        .body
        .people
        .iter()
        .any(|one| one.account == request.account || one.id == request.account);
    if is_in {
        return Err(Error::new(
            ErrorKind::Conflict,
            "that account is already in the org",
        ));
    }
    let mut body = OrgDefinition::successor_of(current, node.now_text());
    let mut person = Person::new(
        request.account.clone(),
        request.label.clone(),
        request.account.clone(),
        request.public_key.clone(),
    )
    .with_department(record.department.clone())
    .with_role(record.role);
    if let Some(seat_id) = &record.seat {
        let seat = take_seat(&mut body, seat_id)?;
        person = person
            .with_department(seat.department)
            .with_ask_about(seat.ask_about)
            .with_formerly(vec![seat.id]);
    }
    body.people.push(person);
    if record.seat.is_some() {
        body.validate().map_err(Error::bad_request)?;
    }
    Ok(body)
}

/// Takes the seat out of the definition and gives it back.
pub(crate) fn take_seat(body: &mut OrgDefinition, id: &Id) -> Result<Seat> {
    let at = body
        .seats
        .iter()
        .position(|one| &one.id == id)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Conflict,
                format!("seat {id} is not in org {} any more", body.org),
            )
        })?;
    Ok(body.seats.remove(at))
}

/// The invite a held join used, for signing it later: its record, still marked used by that joiner.
pub(crate) fn held_invite(node: &Node, request: &JoinRequest) -> Option<InviteRecord> {
    let record = node.store().load_invite(&code_digest(&request.code))?;
    (record.used_by.as_ref() == Some(&request.account)).then_some(record)
}

/// A held join that was refused or expired gives its invite back, unless it expired too.
pub(crate) fn release_held_invite(node: &Node, held: &PendingRootChange) -> Result<()> {
    let RootChange::Join { request } = &held.change else {
        return Ok(());
    };
    let Some(record) = held_invite(node, &request.body) else {
        return Ok(());
    };
    node.store().save_invite(
        &code_digest(&request.body.code),
        &InviteRecord {
            used_by: None,
            ..record
        },
    )
}

/// An org id names one org on a node. A link to an org whose id this node already has, under another
/// root key, is a different org (or a fake one): joining it would replace this node's definition and
/// trust for that id, and with them every team it hosts there. The root key is the one the stored
/// definition names now (it may have been rotated), or else the trusted one.
fn refuse_another_org(node: &Node, invite: &Invite) -> Result<()> {
    let org = &invite.org;
    let stored = node.store().load_org(org);
    let known = match &stored {
        Some(doc) => Some(doc.body.root.public_key.fingerprint()),
        None => node.config().trusted_fingerprint(org).map(str::to_owned),
    };
    match known {
        Some(fingerprint) if fingerprint != invite.fp => Err(Error::new(
            ErrorKind::Conflict,
            format!("this node already has an org {org} with another root key; it cannot join a second org with the same id"),
        )),
        _ => Ok(()),
    }
}

/// Joins an org from an invite link. Stops unless the org's root key matches the fingerprint in the link,
/// and unless `confirm` agrees after seeing the definition.
pub async fn join(
    node: &Node,
    link: &str,
    confirm: impl FnOnce(&OrgDefinition) -> bool,
) -> Result<Joined> {
    let invite = Invite::parse(link).map_err(Error::bad_request)?;
    refuse_another_org(node, &invite)?;
    let access = DefinitionAccess::InviteCode(invite.code.clone());
    let current = node
        .peers()
        .get_definition(&invite.node, &routes::definition(&invite.org), &access)
        .await
        .map_err(|error| {
            Error::new(
                ErrorKind::Unavailable,
                format!("the inviting node did not return the org definition: {error}"),
            )
        })?;
    let root = &current.body.root;
    let is_genuine = current.body.org == invite.org
        && root.public_key.fingerprint() == invite.fp
        && current.signer == root.account
        && current.verify(Kind::Org, &root.public_key);
    if !is_genuine {
        return Err(Error::unauthorized("the org's root key does not match the fingerprint in the link (the node may be fake); not joining"));
    }
    if !confirm(&current.body) {
        return Err(Error::bad_request("joining was cancelled"));
    }
    let config = node.config();
    let request = JoinRequest {
        org: invite.org.clone(),
        code: invite.code.clone(),
        account: config.account.id.clone(),
        label: config.account.label.clone(),
        public_key: node.key().public(),
        at: node.now_text(),
    };
    let signed = Signed::sign(Kind::Join, request, config.account.id.clone(), node.key())
        .map_err(|error| Error::internal(error.to_string()))?;
    let answer = node
        .peers()
        .post_join(&invite.node, &routes::join(&invite.org), &signed)
        .await
        .map_err(|error| {
            Error::new(
                ErrorKind::Forbidden,
                format!("the join was refused: {error}"),
            )
        })?;
    let joined = match answer {
        RootAnswer::Signed(doc) => *doc,
        RootAnswer::Pending(pending) => {
            return wait_for_join(node, &invite, current, pending.pending, pending.message)
        }
    };
    keep_joined(node, &invite, &current, joined).await
}

/// The joiner's side of a join the root's node holds for its owner: the trust and the handle are kept, and
/// `finish_joins` asks for the signed version later.
fn wait_for_join(
    node: &Node,
    invite: &Invite,
    current: SignedOrg,
    pending: String,
    message: String,
) -> Result<Joined> {
    node.store().put_waiting_join(&WaitingJoin {
        org: invite.org.clone(),
        node: invite.node.clone(),
        pending,
        current,
    })?;
    node.update_config(|config| {
        config.trust_with_root_node(invite.org.clone(), invite.fp.clone(), invite.node.clone())
    })?;
    Ok(Joined::Waiting(message))
}

/// Takes the version that added this account, checked against the definition the invite was checked with.
async fn keep_joined(
    node: &Node,
    invite: &Invite,
    current: &SignedOrg,
    joined: SignedOrg,
) -> Result<Joined> {
    let config = node.config();
    let chain = chain_to(node, &invite.node, current, &joined).await?;
    if joined.body.member(&config.account.id).is_none() {
        return Err(Error::bad_request(
            "the returned org definition does not include this account",
        ));
    }
    let _guard = node.write_lock().lock().await;
    refuse_another_org(node, invite)?;
    let stored = node.store().load_org(&invite.org);
    if stored.is_none_or(|stored| stored.body.version < joined.body.version) {
        for version in &chain {
            keep_org(node, version)?;
        }
    }
    node.update_config(|config| {
        config.trust_with_root_node(invite.org.clone(), invite.fp.clone(), invite.node.clone())
    })?;
    node.store().remove_waiting_join(&invite.org)?;
    Ok(Joined::Member(Box::new(joined)))
}

/// The versions from the one after `current` (the definition the invite was checked with) up to `joined`,
/// each checked as the root-signed successor of the one before. Versions signed in between (other joins
/// signed in the same run, or anything else) are read from the root's node, which this node may read now
/// that `joined` names it a member.
async fn chain_to(
    node: &Node,
    origin: &str,
    current: &SignedOrg,
    joined: &SignedOrg,
) -> Result<Vec<SignedOrg>> {
    let mut chain: Vec<SignedOrg> = vec![];
    let mut last = current.clone();
    while last.body.version + 1 < joined.body.version {
        let path = routes::versions(&current.body.org, last.body.version);
        let auth = sign_for_peer(node, &current.body.org, origin, &path);
        let fetched = node
            .peers()
            .get_versions(origin, &path, &auth)
            .await
            .map_err(|error| Error::new(ErrorKind::Unavailable, error.to_string()))?;
        let before = last.body.version;
        for version in fetched.versions {
            if version.body.version >= joined.body.version {
                break;
            }
            verify_root_successor(&last, &version).map_err(Error::bad_request)?;
            last = version.clone();
            chain.push(version);
        }
        if last.body.version == before {
            return Err(Error::new(
                ErrorKind::Unavailable,
                format!("{origin} sent no version after {before}"),
            ));
        }
    }
    verify_root_successor(&last, joined).map_err(Error::bad_request)?;
    chain.push(joined.clone());
    Ok(chain)
}

/// What a join came to.
#[derive(Debug)]
pub enum Joined {
    /// The root's node signed the version with this account in it.
    Member(Box<SignedOrg>),
    /// The root's node holds the join for its owner; the message says what to wait for.
    Waiting(String),
}

/// Asks the root's node of each join that waits whether its owner signed it, and takes the version when
/// they did. A join the root's node no longer knows (refused, or expired) is forgotten.
pub async fn finish_joins(node: &Node) -> Result<Vec<Joined>> {
    let mut done = vec![];
    for waiting in node.store().waiting_joins() {
        let path = routes::held_join(&waiting.org, &waiting.pending);
        let answer = node.peers().get_join(&waiting.node, &path).await;
        let invite = Invite {
            node: waiting.node.clone(),
            org: waiting.org.clone(),
            fp: waiting.current.body.root.public_key.fingerprint(),
            code: String::new(),
        };
        match answer {
            Ok(RootAnswer::Signed(doc)) => {
                done.push(keep_joined(node, &invite, &waiting.current, *doc).await?);
            }
            Ok(RootAnswer::Pending(_)) => {}
            Err(crate::ports::PeerError::Status { status: 404, .. }) => {
                node.store().remove_waiting_join(&waiting.org)?;
            }
            Err(_) => {}
        }
    }
    Ok(done)
}

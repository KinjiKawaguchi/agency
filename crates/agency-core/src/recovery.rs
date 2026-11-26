//! An org's recovery key (docs/protocol.md, "Recovery key"). The root adds it; whoever has its words starts
//! a recovery, which names a new root key made on this node or a new recovery key, and finishes it once this
//! node has held the start for the wait. The recovery key's secret comes in from the words for one call and is
//! never kept.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{self, amend, load};
use agency_proto::org::{PendingRoot, Root, SignedOrg};
use agency_proto::{AccountKey, Id, Kind, OrgDefinition, PublicKey, Signed};

/// What a recovery start moves: the root key to a new one made on this node, or the recovery key to `PublicKey`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovering {
    RootKey,
    RecoveryKey(PublicKey),
}

/// Names `key` as the recovery key of `org`, signed by the root. An org that has one replaces it with its
/// words (`start` with `Recovering::RecoveryKey`), never with the root key.
pub fn add(node: &Node, org: &Id, key: PublicKey) -> Result<SignedOrg> {
    amend(node, org, |body| {
        if body.root.recovery.is_some() {
            return Err(Error::new(
                ErrorKind::Conflict,
                format!("org {org} has a recovery key; replace it with its words"),
            ));
        }
        body.root.recovery = Some(key);
        Ok(())
    })
}

/// Signs `body` with the recovery key of `current`, if `words_key` is it.
fn sign_with_recovery(
    current: &SignedOrg,
    body: OrgDefinition,
    words_key: &AccountKey,
) -> Result<SignedOrg> {
    let root = &current.body.root;
    if root.recovery.as_ref() != Some(&words_key.public()) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!(
                "these words are not the recovery key of org {}",
                current.body.org
            ),
        ));
    }
    Signed::sign(Kind::Org, body, root.account.clone(), words_key)
        .map_err(|error| Error::internal(error.to_string()))
}

/// Takes a version the recovery key signed as any node would (so the wait is counted here too), and sends it
/// to the nodes of the org's teams.
async fn take_and_send(node: &Node, doc: &SignedOrg) -> Result<()> {
    orgs::receive(node, doc.clone()).await?;
    orgs::publish(node, doc)
}

/// Starts a recovery of `org` with the recovery key from its words. Nothing moves until it is finished.
pub async fn start(
    node: &Node,
    org: &Id,
    words_key: &AccountKey,
    what: Recovering,
) -> Result<SignedOrg> {
    let current = load(node, org)?;
    if current.body.root.pending.is_some() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("a recovery of org {org} was started already; finish it"),
        ));
    }
    let new_root = match &what {
        Recovering::RootKey => Some(
            node.root_keys()
                .create(org)
                .map_err(|why| Error::internal(format!("cannot make a root key: {why}")))?,
        ),
        Recovering::RecoveryKey(_) => None,
    };
    let mut body = OrgDefinition::successor_of(&current, node.now_text());
    body.root.pending = Some(PendingRoot {
        public_key: new_root.clone(),
        recovery: match what {
            Recovering::RecoveryKey(key) => Some(key),
            Recovering::RootKey => None,
        },
    });
    let taken = match sign_with_recovery(&current, body, words_key) {
        Ok(doc) => take_and_send(node, &doc).await.map(|()| doc),
        Err(error) => Err(error),
    };
    if let (Err(_), Some(key)) = (&taken, &new_root) {
        let _ = node.root_keys().forget(org, key);
    }
    taken
}

/// Finishes the recovery of `org` that waits, once this node has held its start for the wait.
pub async fn finish(node: &Node, org: &Id, words_key: &AccountKey) -> Result<SignedOrg> {
    let current = load(node, org)?;
    let Some(pending) = current.body.root.pending.clone() else {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("no recovery of org {org} waits"),
        ));
    };
    let root = &current.body.root;
    let mut body = OrgDefinition::successor_of(&current, node.now_text());
    body.root = Root {
        account: root.account.clone(),
        public_key: pending.public_key.unwrap_or(root.public_key.clone()),
        recovery: pending.recovery.or(root.recovery.clone()),
        pending: None,
    };
    let doc = sign_with_recovery(&current, body, words_key)?;
    take_and_send(node, &doc).await?;
    Ok(doc)
}

/// Cancels the recovery of `org` that waits, by signing the next version with the root key: a version the
/// root signs never carries a waiting recovery. It changes nothing else.
pub fn cancel(node: &Node, org: &Id) -> Result<SignedOrg> {
    let current = load(node, org)?;
    if current.body.root.pending.is_none() {
        return Err(Error::new(
            ErrorKind::Conflict,
            format!("no recovery of org {org} waits"),
        ));
    }
    amend(node, org, |_| Ok(()))
}

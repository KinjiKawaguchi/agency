//! Device keys (docs/protocol.md, "Devices"). A new device asks to be added; the person compares pairing
//! codes, and an existing device (or, for the first device, the node's terminal with the account key)
//! signs the change. The home node relays it to the org's root node, which writes the next version.
use crate::error::{Error, ErrorKind, Result};
use crate::node::Node;
use crate::orgs::{is_root, load, publish, sign};
use crate::outbox::{enqueue, outcome_of, settle, Outcome, Parcel, ParcelBody};
use crate::pending_root::{hold, must_hold, RootChange};
use crate::routes;
use crate::unseen;
use agency_proto::client::{
    DeviceChangeOutcome, DeviceChangeResult, DeviceList, DeviceRequest, DeviceRequestInput,
    OrgDevices,
};
use agency_proto::clock::OffsetDateTime;
use agency_proto::clock::{self, Duration};
use agency_proto::device::{check_device_change, device_change_key};
use agency_proto::org::{verify_root_successor, SignedOrg};
use agency_proto::root_change::RootAnswer;
use agency_proto::{
    AccountKey, Device, DeviceChange, DeviceOp, Id, Kind, OrgDefinition, PublicKey, Signed,
    Timestamp,
};
use serde::{Deserialize, Serialize};

/// How long a device request waits to be added.
pub const REQUEST_SECONDS: i64 = 60 * 60;
/// The most device requests kept; older ones are dropped.
pub const MAX_REQUESTS: usize = 10;

/// A device waiting to be added, as this node keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRequestRecord {
    pub label: String,
    pub public_key: PublicKey,
    pub requested_at: String,
}

/// The orgs this node's account belongs to.
fn my_orgs(node: &Node) -> Vec<SignedOrg> {
    let account = node.account();
    let orgs = node.store().orgs().into_iter();
    orgs.filter(|doc| doc.body.member(&account).is_some())
        .collect()
}

pub(crate) fn my_devices<'a>(definition: &'a OrgDefinition, account: &Id) -> &'a [Device] {
    definition
        .member(account)
        .map_or(&[], |person| person.devices.as_slice())
}

/// Whether the key is already one of the account's devices in every org it belongs to.
fn is_added_everywhere(orgs: &[SignedOrg], account: &Id, key: &PublicKey) -> bool {
    orgs.iter().all(|doc| {
        my_devices(&doc.body, account)
            .iter()
            .any(|device| &device.public_key == key)
    })
}

/// Requests that have not expired and are not yet added in every org.
fn open_requests(node: &Node) -> Vec<DeviceRequestRecord> {
    let (orgs, account, now) = (my_orgs(node), node.account(), node.now());
    let is_waiting = |record: &DeviceRequestRecord| {
        let is_recent = clock::parse(&record.requested_at)
            .is_some_and(|at| now - at <= Duration::seconds(REQUEST_SECONDS));
        is_recent && !is_added_everywhere(&orgs, &account, &record.public_key)
    };
    node.store()
        .device_requests()
        .into_iter()
        .filter(is_waiting)
        .collect()
}

fn to_request(record: DeviceRequestRecord) -> Result<DeviceRequest> {
    let requested_at = Timestamp::parse(&record.requested_at)
        .map_err(|_| Error::internal("a device request's time is malformed"))?;
    Ok(DeviceRequest {
        device_id: record.public_key.device_id(),
        pairing_code: record.public_key.pairing_code(),
        label: record.label,
        public_key: record.public_key,
        requested_at,
    })
}

fn is_label(label: &str) -> bool {
    agency_proto::org::label_ok_to_write(label)
}

/// Records a device that asks to be added, and answers with its pairing code.
pub fn request_device(node: &Node, input: DeviceRequestInput) -> Result<DeviceRequest> {
    let label = input.label.trim().to_owned();
    if !is_label(&label) {
        return Err(Error::bad_request("the device label is 1 to 80 characters"));
    }
    if input.public_key == node.key().public() {
        return Err(Error::bad_request("the account key cannot be a device key"));
    }
    let record = DeviceRequestRecord {
        label,
        public_key: input.public_key,
        requested_at: node.now_text(),
    };
    let others = open_requests(node)
        .into_iter()
        .filter(|one| one.public_key != record.public_key);
    let mut kept: Vec<DeviceRequestRecord> = others.collect();
    kept.push(record.clone());
    let start = kept.len().saturating_sub(MAX_REQUESTS);
    node.store().save_device_requests(&kept[start..])?;
    to_request(record)
}

/// The account's devices in each of its orgs, and the devices waiting to be added. `node` is left for the
/// Client API to add, because it depends on who asks and on the running program.
pub fn device_list(node: &Node) -> Result<DeviceList> {
    let account = node.account();
    let orgs = my_orgs(node)
        .into_iter()
        .map(|doc| OrgDevices {
            devices: my_devices(&doc.body, &account).to_vec(),
            org: doc.body.org,
        })
        .collect();
    let requests = open_requests(node)
        .into_iter()
        .map(to_request)
        .collect::<Result<_>>()?;
    Ok(DeviceList {
        orgs,
        requests,
        node: None,
    })
}

/// On the root key's node: checks a device change and writes it into the next version.
pub async fn accept_device_change(
    node: &Node,
    org: &Id,
    signed: Signed<DeviceChange>,
) -> Result<RootAnswer> {
    let _guard = node.write_lock().lock().await;
    let current = node.store().load_org(org);
    let key = current
        .as_ref()
        .and_then(|current| device_change_key(&current.body, &signed));
    let is_valid = unseen::check(&signed, Kind::Device, key);
    let (account, signer) = (&signed.body.account, &signed.signer);
    let current = match current {
        None => {
            return Err(unseen::refused(format!(
                "a device change for {account} in {org}, an org this node does not have"
            )))
        }
        Some(current) if key.is_none() && current.body.member(account).is_none() => {
            return Err(unseen::refused(format!(
                "a device change for {account}, who is not a member of {org}"
            )))
        }
        Some(_) if key.is_none() => {
            return Err(unseen::refused(format!(
                "a device change for {account} signed as {signer}, which is not the key it needs"
            )))
        }
        Some(_) if !is_valid => {
            return Err(unseen::refused(format!(
                "a device change for {account} whose signature does not match {signer}"
            )))
        }
        Some(current) => current,
    };
    if !is_root(node, &current) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            format!("this node does not hold the root key of org {org}"),
        ));
    }
    let body = changed_body(node, &current, &signed, node.now())?;
    if must_hold(node, &current) {
        let change = Box::new(signed);
        return hold(node, org, RootChange::Device { change });
    }
    let doc = sign(node, body)?;
    publish(node, &doc)?;
    Ok(RootAnswer::Signed(Box::new(doc)))
}

/// The next version with the device change applied, checked as it was when it was `received`.
pub(crate) fn changed_body(
    node: &Node,
    current: &SignedOrg,
    signed: &Signed<DeviceChange>,
    received: OffsetDateTime,
) -> Result<OrgDefinition> {
    let changed =
        check_device_change(&current.body, signed, received).map_err(Error::bad_request)?;
    let mut body = OrgDefinition::successor_of(current, node.now_text());
    body.people = changed.people;
    body.revoked = changed.revoked;
    Ok(body)
}

/// Keeps the version the root's node wrote. If this node is more than one version behind, it first
/// takes the versions in between from the root's node.
async fn keep_returned(node: &Node, root_node: &str, returned: SignedOrg) -> Result<()> {
    let org = &returned.body.org;
    if returned.body.version > load(node, org)?.body.version + 1 {
        crate::orgs::catch_up(node, root_node, org, returned.body.version).await?;
    }
    let _guard = node.write_lock().lock().await;
    let current = load(node, &returned.body.org)?;
    if returned.body.version > current.body.version {
        verify_root_successor(&current, &returned).map_err(Error::bad_request)?;
        node.store().save_org(&returned)?;
    }
    Ok(())
}

/// Sends a change to the root's node. A node that cannot be reached gets it later from the outbox.
async fn relay(
    node: &Node,
    root_node: String,
    signed: Signed<DeviceChange>,
) -> Result<DeviceChangeResult> {
    let path = routes::devices(&signed.body.org);
    let parcel = Parcel::new(
        root_node.clone(),
        ParcelBody::Device(Box::new(signed.clone())),
        &node.now_text(),
    );
    enqueue(node, &parcel)?;
    let sent = node
        .peers()
        .post_device_change(&root_node, &path, &signed)
        .await;
    let returned = sent.as_ref().ok().cloned();
    let outcome = outcome_of(sent);
    settle(node, &parcel, &outcome)?;
    match outcome {
        Outcome::Delivered => match returned {
            Some(RootAnswer::Pending(pending)) => Ok(DeviceChangeResult {
                outcome: DeviceChangeOutcome::Pending,
                message: Some(pending.message),
            }),
            Some(RootAnswer::Signed(doc)) => {
                keep_returned(node, &root_node, *doc).await?;
                Ok(DeviceChangeResult {
                    outcome: DeviceChangeOutcome::Delivered,
                    message: None,
                })
            }
            None => Ok(DeviceChangeResult {
                outcome: DeviceChangeOutcome::Delivered,
                message: None,
            }),
        },
        Outcome::Unreachable(message) => Ok(DeviceChangeResult {
            outcome: DeviceChangeOutcome::Queued,
            message: Some(message),
        }),
        Outcome::Refused(message) => Err(Error::new(
            ErrorKind::Conflict,
            format!("the root's node refused: {message}"),
        )),
    }
}

/// Takes a change to one of this node's account's devices, signed by the person, and has the org's root
/// node write it. Checked here first, so a change the root would refuse never enters the outbox.
pub async fn submit_device_change(
    node: &Node,
    signed: Signed<DeviceChange>,
) -> Result<DeviceChangeResult> {
    submit_device_change_as(node, &crate::guests::Caller::Owner, signed).await
}

/// `submit_device_change` for the account the caller acts as: the node's own, or the hosted member a guest is,
/// whose org is the only one it reaches.
pub async fn submit_device_change_as(
    node: &Node,
    caller: &crate::guests::Caller,
    signed: Signed<DeviceChange>,
) -> Result<DeviceChangeResult> {
    if signed.body.account != caller.account(node) || !caller.is_in(&signed.body.org) {
        return Err(Error::new(
            ErrorKind::Forbidden,
            "this node relays only its own account's device changes",
        ));
    }
    let org = signed.body.org.clone();
    let current = node
        .store()
        .load_org(&org)
        .filter(|doc| doc.body.member(&caller.account(node)).is_some())
        .ok_or_else(|| Error::not_found("not found"))?;
    check_device_change(&current.body, &signed, node.now()).map_err(Error::bad_request)?;
    if is_root(node, &current) {
        return Ok(match accept_device_change(node, &org, signed).await? {
            RootAnswer::Signed(_) => DeviceChangeResult {
                outcome: DeviceChangeOutcome::Applied,
                message: None,
            },
            RootAnswer::Pending(pending) => DeviceChangeResult {
                outcome: DeviceChangeOutcome::Pending,
                message: Some(pending.message),
            },
        });
    }
    let root_node = node
        .config()
        .root_node(&org)
        .map(str::to_owned)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Unavailable,
                format!("this node does not know where the root of org {org} runs"),
            )
        })?;
    relay(node, root_node, signed).await
}

/// A device's key and its id, as the node's terminal holds them for the CLI device.
pub struct DeviceSigner<'a> {
    pub id: &'a Id,
    pub key: &'a AccountKey,
}

/// What happened to a device added from the terminal, in one org.
#[derive(Debug)]
pub enum Added {
    Done(DeviceChangeResult),
    /// The account already has devices there, and the CLI device is not one of them: one of them must
    /// add this device.
    NeedsDevice,
    Failed(Error),
}

/// Who signs an add in this org: the account key while the account has no device there, then the CLI
/// device if it is one of the account's devices there.
fn add_signer<'a>(
    node: &'a Node,
    definition: &OrgDefinition,
    cli: Option<&DeviceSigner<'a>>,
) -> Option<(Id, &'a AccountKey)> {
    let devices = my_devices(definition, &node.account());
    if devices.is_empty() {
        return Some((node.account(), node.key()));
    }
    let cli = cli?;
    let is_added = devices
        .iter()
        .any(|device| device.id == *cli.id && device.public_key == cli.key.public());
    is_added.then(|| (cli.id.clone(), cli.key))
}

/// Adds a device from the node's terminal (`agency device-add`), in every org the account belongs to.
/// The account key signs only a first device; after that, the CLI device signs, if it was added. No
/// Client API route signs with the account key or the CLI device.
pub async fn add_device(
    node: &Node,
    label: &str,
    public_key: &PublicKey,
    cli: Option<&DeviceSigner<'_>>,
) -> Vec<(Id, Added)> {
    let mut results = vec![];
    for doc in my_orgs(node) {
        let org = doc.body.org.clone();
        let Some((signer, key)) = add_signer(node, &doc.body, cli) else {
            results.push((org, Added::NeedsDevice));
            continue;
        };
        let outcome = match add_change(node, &org, label, public_key, signer, key) {
            Ok(signed) => match submit_device_change(node, signed).await {
                Ok(result) => Added::Done(result),
                Err(error) => Added::Failed(error),
            },
            Err(error) => Added::Failed(error),
        };
        results.push((org, outcome));
    }
    results
}

fn add_change(
    node: &Node,
    org: &Id,
    label: &str,
    public_key: &PublicKey,
    signer: Id,
    key: &AccountKey,
) -> Result<Signed<DeviceChange>> {
    let at =
        Timestamp::parse(&node.now_text()).map_err(|error| Error::internal(error.to_string()))?;
    let device = Device {
        id: public_key.device_id(),
        label: label.to_owned(),
        public_key: public_key.clone(),
        added_at: at.clone(),
    };
    let change = DeviceChange {
        org: org.clone(),
        account: node.account(),
        op: DeviceOp::Add {
            device: Box::new(device),
        },
        at,
    };
    Signed::sign(Kind::Device, change, signer, key)
        .map_err(|error| Error::internal(error.to_string()))
}

/// Open requests, for the terminal to list.
pub fn pending_requests(node: &Node) -> Result<Vec<DeviceRequest>> {
    open_requests(node).into_iter().map(to_request).collect()
}

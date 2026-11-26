//! What a version changes, in words, for the owner's presence prompt: "acme: add member carol (Carol)",
//! "acme: replace the key of member bob", "acme: move the root to a new key". Every kind of change the
//! version makes is named, or at least counted by kind: a prompt that leaves a change out teaches the owner
//! that it signs only what it says.
use agency_proto::org::{printable, Person, Role};
use agency_proto::{Id, OrgDefinition};

/// The most changes named one by one; the rest are counted by kind.
const MOST_NAMED: usize = 4;

/// One change: its kind ("member", "device", "team", ...) and its words.
struct Change {
    kind: &'static str,
    words: String,
}

fn change(kind: &'static str, words: String) -> Change {
    Change { kind, words }
}

/// The entries of one list in the definition, compared whole by id: added, removed, or changed in any field.
fn compare<T: PartialEq>(
    kind: &'static str,
    before: &[T],
    after: &[T],
    id_of: impl Fn(&T) -> &Id,
    label_of: impl Fn(&T) -> &str,
    changes: &mut Vec<Change>,
) {
    for entry in after {
        let id = id_of(entry);
        match before.iter().find(|one| id_of(one) == id) {
            None => changes.push(change(
                kind,
                format!("add {kind} {id} ({})", printable(label_of(entry))),
            )),
            Some(old) if old != entry => changes.push(change(kind, format!("change {kind} {id}"))),
            Some(_) => {}
        }
    }
    for entry in before {
        let id = id_of(entry);
        if !after.iter().any(|one| id_of(one) == id) {
            changes.push(change(kind, format!("remove {kind} {id}")));
        }
    }
}

/// Members, with words of their own for what matters most: a new key, a new role, and devices.
fn member_changes(before: &OrgDefinition, after: &OrgDefinition, changes: &mut Vec<Change>) {
    for person in &after.people {
        let Some(old) = before.people.iter().find(|one| one.id == person.id) else {
            let role = if person.role == Role::Answerer {
                " as answerer"
            } else {
                ""
            };
            let label = printable(&person.label);
            changes.push(change(
                "member",
                format!("add member {} ({label}){role}", person.id),
            ));
            continue;
        };
        if old.public_key != person.public_key || old.account != person.account {
            changes.push(change(
                "member",
                format!("replace the key of member {}", person.id),
            ));
        }
        if old.role != person.role {
            let role = if person.role == Role::Answerer {
                "an answerer"
            } else {
                "a member"
            };
            changes.push(change("member", format!("make {} {role}", person.id)));
        }
        if (&old.label, &old.department) != (&person.label, &person.department) {
            changes.push(change("member", format!("change member {}", person.id)));
        }
        device_changes(old, person, changes);
    }
    for old in &before.people {
        if !after.people.iter().any(|one| one.id == old.id) {
            changes.push(change("member", format!("remove member {}", old.id)));
        }
    }
}

fn device_changes(old: &Person, person: &Person, changes: &mut Vec<Change>) {
    for device in &person.devices {
        match old.devices.iter().find(|one| one.id == device.id) {
            None => {
                let label = printable(&device.label);
                changes.push(change(
                    "device",
                    format!("add device {label} for {}", person.id),
                ));
            }
            Some(was) if was.public_key != device.public_key => changes.push(change(
                "device",
                format!("replace the key of device {} of {}", device.id, person.id),
            )),
            Some(was) if was != device => changes.push(change(
                "device",
                format!("change device {} of {}", device.id, person.id),
            )),
            Some(_) => {}
        }
    }
    for device in &old.devices {
        if !person.devices.iter().any(|one| one.id == device.id) {
            let words = format!("remove device {} of {}", device.id, person.id);
            changes.push(change("device", words));
        }
    }
}

fn root_changes(before: &OrgDefinition, after: &OrgDefinition, changes: &mut Vec<Change>) {
    let (old, new) = (&before.root, &after.root);
    if old.public_key != new.public_key {
        changes.push(change("root", "move the root to a new key".into()));
    }
    if old.recovery != new.recovery {
        let words = if old.recovery.is_none() {
            "add a recovery key"
        } else {
            "replace the recovery key"
        };
        changes.push(change("root", words.into()));
    }
    if old.pending.is_some() && new.pending.is_none() && old.public_key == new.public_key {
        changes.push(change("root", "cancel the waiting recovery".into()));
    }
    if old.account != new.account {
        changes.push(change("root", format!("make {} the root", new.account)));
    }
}

fn all_changes(before: &OrgDefinition, after: &OrgDefinition) -> Vec<Change> {
    let mut changes = vec![];
    if before.label != after.label {
        let words = format!("rename the org to {}", printable(&after.label));
        changes.push(change("org", words));
    }
    root_changes(before, after, &mut changes);
    member_changes(before, after, &mut changes);
    compare(
        "department",
        &before.departments,
        &after.departments,
        |one| &one.id,
        |one| &one.label,
        &mut changes,
    );
    compare(
        "team",
        &before.teams,
        &after.teams,
        |one| &one.id,
        |one| &one.label,
        &mut changes,
    );
    compare(
        "KPI",
        &before.kpis,
        &after.kpis,
        |one| &one.id,
        |one| &one.label,
        &mut changes,
    );
    compare(
        "seat",
        &before.seats,
        &after.seats,
        |one| &one.id,
        |one| &one.label,
        &mut changes,
    );
    compare(
        "external party",
        &before.externals,
        &after.externals,
        |one| &one.id,
        |one| &one.label,
        &mut changes,
    );
    let revoked = after
        .revoked
        .iter()
        .filter(|key| !before.revoked.contains(key))
        .count();
    if revoked > 0 {
        changes.push(change("revoked key", format!("revoke {revoked} key(s)")));
    }
    changes
}

/// The changes from `before` to `after`, for org `after.org`: up to `MOST_NAMED` named one by one, and the
/// rest counted with their kinds, so nothing a press signs goes unsaid.
pub fn describe(before: Option<&OrgDefinition>, after: &OrgDefinition) -> String {
    let org = &after.org;
    let Some(before) = before else {
        return format!("{org}: create the org {}", printable(&after.label));
    };
    let changes = all_changes(before, after);
    if changes.is_empty() {
        return format!("{org}: sign the next version, which changes nothing else");
    }
    let named: Vec<&str> = changes
        .iter()
        .take(MOST_NAMED)
        .map(|one| one.words.as_str())
        .collect();
    let rest = &changes[named.len()..];
    let mut words = format!("{org}: {}", named.join(", "));
    if !rest.is_empty() {
        let mut kinds: Vec<&str> = rest.iter().map(|one| one.kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        words.push_str(&format!(
            ", and {} other changes: {}",
            rest.len(),
            kinds.join(", ")
        ));
    }
    words
}

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
        let named_before = changes.len();
        if (&old.label, &old.department) != (&person.label, &person.department) {
            changes.push(change("member", format!("change member {}", person.id)));
        }
        // Any other signed field of a member (one added later, too) is still said.
        let without_devices = |one: &Person| {
            let mut kept = one.clone();
            kept.devices.clear();
            kept
        };
        let is_named = changes.len() > named_before
            || old.public_key != person.public_key
            || old.account != person.account
            || old.role != person.role;
        if !is_named && without_devices(old) != without_devices(person) {
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
    if new.pending.is_some() && old.pending != new.pending {
        changes.push(change("root", "start a recovery".into()));
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

/// Every change from `before` to `after` in words, one each, none left out: what an audit of the org lists
/// (`audit`). A change these words do not know yet is said as such, never left unsaid.
pub fn every_change(before: &OrgDefinition, after: &OrgDefinition) -> Vec<String> {
    let changes: Vec<String> = all_changes(before, after)
        .into_iter()
        .map(|one| one.words)
        .collect();
    if !changes.is_empty() {
        return changes;
    }
    let mut same_place = after.clone();
    same_place.version = before.version;
    same_place.prev = before.prev.clone();
    same_place.at = before.at.clone();
    if &same_place == before {
        return vec![];
    }
    vec!["change something these words do not name".to_owned()]
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
        // A kind of change these words do not know yet is never left unsaid.
        let mut same_place = after.clone();
        same_place.version = before.version;
        same_place.prev = before.prev.clone();
        same_place.at = before.at.clone();
        if &same_place != before {
            return format!("{org}: change something this prompt does not name");
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use agency_proto::org::Root;
    use agency_proto::AccountKey;

    fn org() -> OrgDefinition {
        let alice = Id::parse("alice").unwrap();
        let root = Root::new(alice, AccountKey::from_seed([1; 32]).public());
        let at = "2026-11-24T12:00:00Z".to_owned();
        OrgDefinition::new(Id::parse("acme").unwrap(), "Acme".into(), root, at)
    }

    #[test]
    fn every_change_says_a_change_no_words_name_and_says_nothing_for_none() {
        let mut before = org();
        before.revoked = vec![AccountKey::from_seed([2; 32]).public()];
        let mut after = before.clone();
        after.version = 2;
        after.revoked.clear();
        assert_eq!(
            every_change(&before, &after),
            ["change something these words do not name"]
        );
        let mut same = before.clone();
        same.version = 2;
        assert!(every_change(&before, &same).is_empty());
    }

    #[test]
    fn a_change_no_words_name_is_still_said() {
        let mut before = org();
        before.revoked = vec![AccountKey::from_seed([2; 32]).public()];
        let mut after = before.clone();
        after.version = 2;
        after.revoked.clear();
        let said = describe(Some(&before), &after);
        assert_eq!(said, "acme: change something this prompt does not name");
        let mut same = before.clone();
        same.version = 2;
        let said = describe(Some(&before), &same);
        assert_eq!(
            said,
            "acme: sign the next version, which changes nothing else"
        );
    }
}

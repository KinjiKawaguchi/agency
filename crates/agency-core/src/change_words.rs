//! What a version changes, in words, for the owner's presence prompt: "acme: add member carol (Carol)",
//! "acme: move the root to a new key". A prompt that says only "Sign version 7" teaches the owner to press
//! without reading.
use agency_proto::org::printable;
use agency_proto::{Id, OrgDefinition};

/// The most changes named; the rest are counted.
const MOST_NAMED: usize = 4;

/// One kind of item in the definition, by id and label.
fn compare(kind: &str, before: &[(Id, String)], after: &[(Id, String)], words: &mut Vec<String>) {
    for (id, label) in after {
        match before.iter().find(|(one, _)| one == id) {
            None => words.push(format!("add {kind} {id} ({})", printable(label))),
            Some((_, old)) if old != label => {
                words.push(format!("rename {kind} {id} to {}", printable(label)));
            }
            Some(_) => {}
        }
    }
    for (id, _) in before {
        if !after.iter().any(|(one, _)| one == id) {
            words.push(format!("remove {kind} {id}"));
        }
    }
}

/// The ids and labels of a list in the definition.
fn named<T>(items: &[T], of: impl Fn(&T) -> (&Id, &str)) -> Vec<(Id, String)> {
    items
        .iter()
        .map(|one| {
            let (id, label) = of(one);
            (id.clone(), label.to_owned())
        })
        .collect()
}

fn root_words(before: &OrgDefinition, after: &OrgDefinition, words: &mut Vec<String>) {
    let (old, new) = (&before.root, &after.root);
    if old.public_key != new.public_key {
        words.push("move the root to a new key".into());
    }
    if old.recovery.is_none() && new.recovery.is_some() {
        words.push("add a recovery key".into());
    }
    if old.pending.is_some() && new.pending.is_none() && old.public_key == new.public_key {
        words.push("cancel the waiting recovery".into());
    }
}

fn device_words(before: &OrgDefinition, after: &OrgDefinition, words: &mut Vec<String>) {
    for person in &after.people {
        let old = before.people.iter().find(|one| one.id == person.id);
        let old_devices = old.map(|one| one.devices.as_slice()).unwrap_or_default();
        for device in &person.devices {
            if !old_devices.iter().any(|one| one.id == device.id) {
                let label = printable(&device.label);
                words.push(format!("add device {label} for {}", person.id));
            }
        }
        for device in old_devices {
            if !person.devices.iter().any(|one| one.id == device.id) {
                words.push(format!("remove device {} of {}", device.id, person.id));
            }
        }
    }
}

/// The changes from `before` to `after`, named one by one up to `MOST_NAMED`, for org `after.org`.
pub fn describe(before: Option<&OrgDefinition>, after: &OrgDefinition) -> String {
    let org = &after.org;
    let Some(before) = before else {
        return format!("{org}: create the org {}", printable(&after.label));
    };
    let mut words = vec![];
    if before.label != after.label {
        words.push(format!("rename the org to {}", printable(&after.label)));
    }
    root_words(before, after, &mut words);
    let lists = [
        (
            "department",
            named(&before.departments, |one| (&one.id, &one.label)),
            named(&after.departments, |one| (&one.id, &one.label)),
        ),
        (
            "member",
            named(&before.people, |one| (&one.id, &one.label)),
            named(&after.people, |one| (&one.id, &one.label)),
        ),
        (
            "team",
            named(&before.teams, |one| (&one.id, &one.label)),
            named(&after.teams, |one| (&one.id, &one.label)),
        ),
        (
            "KPI",
            named(&before.kpis, |one| (&one.id, &one.label)),
            named(&after.kpis, |one| (&one.id, &one.label)),
        ),
        (
            "seat",
            named(&before.seats, |one| (&one.id, &one.label)),
            named(&after.seats, |one| (&one.id, &one.label)),
        ),
        (
            "external party",
            named(&before.externals, |one| (&one.id, &one.label)),
            named(&after.externals, |one| (&one.id, &one.label)),
        ),
    ];
    for (kind, old, new) in &lists {
        compare(kind, old, new, &mut words);
    }
    device_words(before, after, &mut words);
    if words.is_empty() {
        words.push("change what a team plans or other settings".into());
    }
    let more = words.len().saturating_sub(MOST_NAMED);
    words.truncate(MOST_NAMED);
    let tail = if more > 0 {
        format!(", and {more} more")
    } else {
        String::new()
    };
    format!("{org}: {}{tail}", words.join(", "))
}

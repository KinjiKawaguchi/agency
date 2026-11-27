//! Seats and external parties in an org definition (docs/protocol.md, "Org definition"; docs/glossary.md, "Seat",
//! "External party", "Answer for").
//!
//! A seat is a place in the org that has a name and a department and that no one has taken yet; an external
//! party is someone the org asks through a member and who never takes a seat. Neither has an account, a key, or
//! a device, so neither is a member: every rule that keys on `people[]` (membership, signed reads, devices,
//! envelopes) cannot reach them, and they never decide. A request addressed to one is answered for it by one of
//! the members named in `relays`. The definition holds labels and what to ask about only: never an address, an
//! email or a phone number.
use crate::ids::Id;
use crate::org::{label_ok, OrgDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use ts_rs::TS;

/// The most seats, and the most external parties, an org states.
pub const MAX_SEATS: usize = 200;
pub const MAX_EXTERNALS: usize = 200;
/// The most members named to answer for one seat or external party.
pub const MAX_RELAYS: usize = 10;
/// The most things a seat or an external party is asked about, and the longest of them, in characters.
pub const MAX_ASK_ABOUT: usize = 20;
pub const MAX_ASK_ABOUT_LENGTH: usize = 200;
/// The most seats one member took.
pub const MAX_FORMERLY: usize = 10;

/// A place in the org that no one has taken yet. When someone takes it, the seat is gone and its id is kept
/// in that member's `formerly`, so the requests and the history that named it stay attached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct Seat {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub department: Option<Id>,
    /// What to ask this seat about, in the org's own words.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub ask_about: Vec<String>,
    /// The members who may answer for the seat, in order of preference. With none, the team's owner does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<Id>>", optional)]
    pub relays: Vec<Id>,
}

/// Someone the org works with who never takes a seat, for example outside counsel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export, optional_fields)]
pub struct External {
    pub id: Id,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub ask_about: Vec<String>,
    /// The members who may answer for them, in order of preference. At least one: an external party answers
    /// nothing in Agency.
    pub relays: Vec<Id>,
}

/// The rules for seats and external parties: checked by `OrgDefinition::validate`, so every node refuses a bad
/// version the same way. There are at most `MAX_SEATS` seats and `MAX_EXTERNALS` external parties; an id is
/// used once among members, the seats they took (`formerly`, at most `MAX_FORMERLY` each), seats and external
/// parties (so it survives a seat being taken); a seat's department is one of the org; whoever answers for
/// one is a member (a seat answers for no one), named once, at most `MAX_RELAYS` of them, and an external
/// party has at least one; what to ask about, of a seat, an external party or a member, is at most
/// `MAX_ASK_ABOUT` texts of 1 to `MAX_ASK_ABOUT_LENGTH` characters without control characters.
pub(crate) fn validate(definition: &OrgDefinition) -> Result<(), String> {
    if definition.seats.len() > MAX_SEATS {
        return Err(format!("an org states at most {MAX_SEATS} seats"));
    }
    if definition.externals.len() > MAX_EXTERNALS {
        return Err(format!(
            "an org states at most {MAX_EXTERNALS} external parties"
        ));
    }
    let mut used: HashSet<&Id> = definition.people.iter().map(|one| &one.id).collect();
    let taken = definition.people.iter().flat_map(|one| &one.formerly);
    let named = taken
        .chain(definition.seats.iter().map(|one| &one.id))
        .chain(definition.externals.iter().map(|one| &one.id));
    for id in named {
        if !used.insert(id) {
            return Err(format!(
                "{id} is the id of more than one member, seat, or external party"
            ));
        }
    }
    for person in &definition.people {
        if person.formerly.len() > MAX_FORMERLY {
            return Err(format!("{} took at most {MAX_FORMERLY} seats", person.id));
        }
        check_ask_about(&person.id, &person.ask_about)?;
    }
    for seat in &definition.seats {
        if !label_ok(&seat.label) {
            return Err(format!("malformed label of seat {}", seat.id));
        }
        if let Some(department) = &seat.department {
            if !definition
                .departments
                .iter()
                .any(|one| &one.id == department)
            {
                return Err(format!(
                    "seat {} is in department {department}, which the org does not have",
                    seat.id
                ));
            }
        }
        check_ask_about(&seat.id, &seat.ask_about)?;
        check_relays(definition, &seat.id, &seat.relays)?;
    }
    for external in &definition.externals {
        if !label_ok(&external.label) {
            return Err(format!("malformed label of external party {}", external.id));
        }
        if external.relays.is_empty() {
            return Err(format!(
                "external party {} needs at least one member to answer for them",
                external.id
            ));
        }
        check_ask_about(&external.id, &external.ask_about)?;
        check_relays(definition, &external.id, &external.relays)?;
    }
    Ok(())
}

fn check_ask_about(whose: &Id, ask_about: &[String]) -> Result<(), String> {
    let is_text = |text: &String| {
        let length = text.chars().count();
        (1..=MAX_ASK_ABOUT_LENGTH).contains(&length) && !text.chars().any(char::is_control)
    };
    if ask_about.len() > MAX_ASK_ABOUT || !ask_about.iter().all(is_text) {
        return Err(format!(
            "what to ask {whose} about is at most {MAX_ASK_ABOUT} texts of 1 to {MAX_ASK_ABOUT_LENGTH} characters without control characters"
        ));
    }
    Ok(())
}

fn check_relays(definition: &OrgDefinition, whose: &Id, relays: &[Id]) -> Result<(), String> {
    if relays.len() > MAX_RELAYS {
        return Err(format!(
            "{whose} is answered for by at most {MAX_RELAYS} members"
        ));
    }
    let mut seen = HashSet::new();
    for member in relays {
        if !definition.people.iter().any(|one| &one.id == member) {
            return Err(format!(
                "{whose} is answered for by {member}, who is not a member"
            ));
        }
        if !seen.insert(member) {
            return Err(format!("{whose} is answered for by {member} twice"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::org::{Department, Person, Root};
    use crate::AccountKey;

    fn id(raw: &str) -> Id {
        Id::parse(raw).unwrap()
    }

    fn member(raw: &str, seed: u8) -> Person {
        Person::new(
            id(raw),
            raw.to_uppercase(),
            id(raw),
            AccountKey::from_seed([seed; 32]).public(),
        )
    }

    /// An org with the members alice and bob and the department dev, and nothing else yet.
    fn org() -> OrgDefinition {
        let root = Root::new(id("alice"), AccountKey::from_seed([1; 32]).public());
        let dev = Department {
            id: id("dev"),
            label: "Dev".into(),
            parent: None,
            head: None,
        };
        OrgDefinition::new(
            id("acme"),
            "Acme".into(),
            root,
            "2026-11-24T12:00:00Z".into(),
        )
        .with_departments(vec![dev])
        .with_people(vec![member("alice", 2), member("bob", 3)])
    }

    fn seat(raw: &str) -> Seat {
        Seat {
            id: id(raw),
            label: "Carol (finance)".into(),
            department: Some(id("dev")),
            ask_about: vec!["invoices".into()],
            relays: vec![id("alice")],
        }
    }

    fn external(raw: &str) -> External {
        External {
            id: id(raw),
            label: "Erin (outside counsel)".into(),
            ask_about: vec!["legal checks".into()],
            relays: vec![id("alice"), id("bob")],
        }
    }

    fn refused(edit: impl FnOnce(&mut OrgDefinition)) -> String {
        let mut definition = org();
        edit(&mut definition);
        definition.validate().unwrap_err()
    }

    #[test]
    fn a_seat_and_an_external_party_are_accepted_and_a_bare_seat_is_just_an_id_and_a_label() {
        let mut definition = org();
        definition.seats.push(seat("carol"));
        definition.externals.push(external("erin"));
        assert_eq!(definition.validate(), Ok(()));
        let bare = Seat {
            id: id("dan"),
            label: "Dan".into(),
            department: None,
            ask_about: vec![],
            relays: vec![],
        };
        assert_eq!(
            serde_json::to_value(&bare).unwrap(),
            serde_json::json!({ "id": "dan", "label": "Dan" })
        );
        definition.seats.push(bare);
        assert_eq!(
            definition.validate(),
            Ok(()),
            "a seat with no one to answer for it falls to the team's owner"
        );
    }

    #[test]
    fn an_id_is_used_once_among_members_seats_and_external_parties() {
        assert!(
            refused(|d| d.seats.push(seat("bob"))).contains("bob"),
            "a seat with a member's id"
        );
        assert!(refused(|d| d.externals.push(external("alice"))).contains("alice"));
        assert!(refused(|d| d.seats.extend([seat("carol"), seat("carol")])).contains("carol"));
        assert!(refused(|d| {
            d.seats.push(seat("erin"));
            d.externals.push(external("erin"));
        })
        .contains("erin"));
    }

    #[test]
    fn a_seat_is_in_a_department_of_the_org() {
        let said = refused(|d| {
            d.seats.push(Seat {
                department: Some(id("sales")),
                ..seat("carol")
            })
        });
        assert!(said.contains("carol") && said.contains("sales"), "{said}");
    }

    #[test]
    fn whoever_answers_for_a_seat_or_an_external_party_is_a_member_named_once() {
        let unknown = refused(|d| {
            d.seats.push(Seat {
                relays: vec![id("zed")],
                ..seat("carol")
            })
        });
        assert!(
            unknown.contains("carol") && unknown.contains("zed"),
            "{unknown}"
        );
        let a_seat = refused(|d| {
            d.seats.push(seat("carol"));
            d.seats.push(Seat {
                relays: vec![id("carol")],
                ..seat("dan")
            });
        });
        assert!(
            a_seat.contains("dan") && a_seat.contains("carol"),
            "a seat answers for no one: {a_seat}"
        );
        assert!(refused(|d| d.externals.push(External {
            relays: vec![id("zed")],
            ..external("erin")
        }))
        .contains("zed"));
        let twice = refused(|d| {
            d.seats.push(Seat {
                relays: vec![id("bob"), id("bob")],
                ..seat("carol")
            })
        });
        assert!(twice.contains("twice"), "{twice}");
    }

    #[test]
    fn an_external_party_has_at_least_one_member_to_answer_for_them() {
        let said = refused(|d| {
            d.externals.push(External {
                relays: vec![],
                ..external("erin")
            })
        });
        assert!(
            said.contains("erin") && said.contains("at least one"),
            "{said}"
        );
    }

    #[test]
    fn no_more_than_ten_members_answer_for_one() {
        let many = |count: usize| {
            let mut definition = org();
            let members: Vec<Person> = (0..count)
                .map(|n| member(&format!("m{n}"), 10 + n as u8))
                .collect();
            let names: Vec<Id> = members.iter().map(|one| one.id.clone()).collect();
            definition.people.extend(members);
            definition.seats.push(Seat {
                relays: names,
                ..seat("carol")
            });
            definition.validate()
        };
        assert_eq!(many(MAX_RELAYS), Ok(()));
        assert!(many(MAX_RELAYS + 1).unwrap_err().contains("at most"));
    }

    #[test]
    fn what_to_ask_about_is_a_few_short_texts() {
        let ask = |texts: Vec<String>| {
            refused(|d| {
                d.seats.push(Seat {
                    ask_about: texts,
                    ..seat("carol")
                })
            })
        };
        assert!(
            ask(vec!["x".into(); MAX_ASK_ABOUT + 1]).contains("carol"),
            "too many"
        );
        assert!(
            ask(vec!["x".repeat(MAX_ASK_ABOUT_LENGTH + 1)]).contains("carol"),
            "too long"
        );
        assert!(ask(vec![String::new()]).contains("carol"), "empty");
        assert!(
            ask(vec!["a\nb".into()]).contains("carol"),
            "a control character"
        );
        let mut definition = org();
        let longest = "x".repeat(MAX_ASK_ABOUT_LENGTH);
        definition.seats.push(Seat {
            ask_about: vec![longest; MAX_ASK_ABOUT],
            ..seat("carol")
        });
        assert_eq!(definition.validate(), Ok(()), "the limits are inclusive");
    }

    #[test]
    fn a_member_keeps_the_ids_of_the_seats_they_took_and_no_one_else_may_use_them() {
        let mut definition = org();
        definition.people[1].formerly = vec![id("carol")];
        definition.people[1].ask_about = vec!["invoices".into()];
        assert_eq!(definition.validate(), Ok(()));
        let json = serde_json::to_value(&definition.people[1]).unwrap();
        assert_eq!(json["formerly"], serde_json::json!(["carol"]));
        assert_eq!(json["askAbout"], serde_json::json!(["invoices"]));
        let bare = serde_json::to_value(&definition.people[0]).unwrap();
        assert!(bare.get("formerly").is_none() && bare.get("askAbout").is_none());

        let by_a_seat = refused(|d| {
            d.people[1].formerly = vec![id("carol")];
            d.seats.push(seat("carol"));
        });
        assert!(by_a_seat.contains("carol"), "{by_a_seat}");
        let by_an_external = refused(|d| {
            d.people[1].formerly = vec![id("erin")];
            d.externals.push(external("erin"));
        });
        assert!(by_an_external.contains("erin"), "{by_an_external}");
        let by_a_member = refused(|d| d.people[1].formerly = vec![id("alice")]);
        assert!(by_a_member.contains("alice"), "{by_a_member}");
        let by_two = refused(|d| {
            d.people[0].formerly = vec![id("carol")];
            d.people[1].formerly = vec![id("carol")];
        });
        assert!(by_two.contains("carol"), "{by_two}");
        let twice = refused(|d| d.people[1].formerly = vec![id("carol"), id("carol")]);
        assert!(twice.contains("carol"), "{twice}");
    }

    #[test]
    fn a_member_took_at_most_ten_seats_and_is_asked_about_a_few_short_texts() {
        let took = |count: usize| {
            let mut definition = org();
            definition.people[1].formerly = (0..count).map(|n| id(&format!("s{n}"))).collect();
            definition.validate()
        };
        assert_eq!(took(MAX_FORMERLY), Ok(()));
        let said = took(MAX_FORMERLY + 1).unwrap_err();
        assert!(said.contains("bob") && said.contains("at most"), "{said}");
        let asked = refused(|d| d.people[1].ask_about = vec!["a\nb".into()]);
        assert!(asked.contains("bob"), "{asked}");
        let many = refused(|d| d.people[1].ask_about = vec!["x".into(); MAX_ASK_ABOUT + 1]);
        assert!(many.contains("bob"), "{many}");
    }

    #[test]
    fn a_label_is_well_formed_and_the_counts_are_limited() {
        assert!(refused(|d| d.seats.push(Seat {
            label: String::new(),
            ..seat("carol")
        }))
        .contains("label"));
        assert!(refused(|d| d.externals.push(External {
            label: "a\tb".into(),
            ..external("erin")
        }))
        .contains("label"));
        let many_seats =
            refused(|d| d.seats = (0..=MAX_SEATS).map(|n| seat(&format!("s{n}"))).collect());
        assert!(many_seats.contains("at most"), "{many_seats}");
        let many_externals = refused(|d| {
            d.externals = (0..=MAX_EXTERNALS)
                .map(|n| external(&format!("e{n}")))
                .collect()
        });
        assert!(many_externals.contains("at most"), "{many_externals}");
    }

    #[test]
    fn a_field_this_release_does_not_know_is_refused_in_a_seat_and_in_an_external_party() {
        let seat =
            serde_json::json!({ "id": "carol", "label": "Carol", "email": "carol@example.com" });
        assert!(
            serde_json::from_value::<Seat>(seat).is_err(),
            "no contact details in the definition"
        );
        let external =
            serde_json::json!({ "id": "erin", "label": "Erin", "relays": ["alice"], "phone": "1" });
        assert!(serde_json::from_value::<External>(external).is_err());
    }
}

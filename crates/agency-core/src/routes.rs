//! The federation routes (`/fed/v1`). Read signatures cover the path, so both sides build it here.
//! Where a route takes a query, the signed path includes it.
use agency_proto::Id;

pub fn definition(org: &Id) -> String {
    format!("/fed/v1/orgs/{org}/definition")
}

/// The versions of an org's definition after `after`. The query is part of the signed path.
pub fn versions(org: &Id, after: u64) -> String {
    format!("/fed/v1/orgs/{org}/versions?after={after}")
}

pub fn devices(org: &Id) -> String {
    format!("/fed/v1/orgs/{org}/devices")
}

pub fn join(org: &Id) -> String {
    format!("/fed/v1/orgs/{org}/join")
}

pub fn view(org: &Id, team: &Id) -> String {
    format!("/fed/v1/teams/{org}/{team}/view")
}

pub fn envelopes(org: &Id, team: &Id) -> String {
    format!("/fed/v1/teams/{org}/{team}/envelopes")
}

/// The query is part of the signed path, so a signature for one range cannot read another.
pub fn evaluation(org: &Id, team: &Id, days: u32) -> String {
    format!("/fed/v1/teams/{org}/{team}/evaluation?days={days}")
}

/// The query is part of the signed path, so a signature for one range cannot read another.
pub fn usage(org: &Id, team: &Id, days: u32) -> String {
    format!("/fed/v1/teams/{org}/{team}/usage?days={days}")
}

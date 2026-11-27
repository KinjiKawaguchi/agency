//! Telling the teams of a shared repository apart: a team that names a board field and an option counts
//! only the pull requests whose issue has that option (docs/protocol.md, "Evaluation").
use super::facts::MergedPullRequest;
use crate::config::DeliveryConfig;

/// The option of the board field `field` on the issues `pull_request` closes: the first one the board lists.
/// The field's name and the option are compared without regard to case.
fn board_option<'a>(pull_request: &'a MergedPullRequest, field: &str) -> Option<&'a str> {
    pull_request
        .board
        .iter()
        .find(|one| one.field.trim().eq_ignore_ascii_case(field.trim()))
        .map(|one| one.value.as_str())
}

/// The pull requests that count for a team with this `delivery` setting: all of them when it names no team
/// field; otherwise those whose issue has the team's option. A pull request that closes no issue, or an issue
/// with no option in the field, counts for no team.
pub fn of_team(
    delivery: Option<&DeliveryConfig>,
    pull_requests: Vec<MergedPullRequest>,
) -> Vec<MergedPullRequest> {
    let Some((field, value)) = delivery.and_then(|delivery| {
        Some((
            delivery.team_field.as_deref()?,
            delivery.team_value.as_deref()?,
        ))
    }) else {
        return pull_requests;
    };
    pull_requests
        .into_iter()
        .filter(|one| {
            board_option(one, field)
                .is_some_and(|option| option.trim().eq_ignore_ascii_case(value.trim()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delivery::BoardValue;
    use agency_proto::clock;

    fn pull_request(number: u64, board: &[(&str, &str)]) -> MergedPullRequest {
        let at = clock::parse("2026-11-24T09:00:00Z").unwrap();
        MergedPullRequest {
            repo: "acme/app".into(),
            number,
            title: "feat: x".into(),
            body: String::new(),
            opened_at: at,
            merged_at: at,
            first_commit_at: None,
            has_issue: !board.is_empty(),
            issue_started_at: None,
            closes_incident: false,
            work_type: None,
            board: board
                .iter()
                .map(|(field, value)| BoardValue {
                    field: (*field).into(),
                    value: (*value).into(),
                })
                .collect(),
        }
    }

    fn numbers(delivery: Option<&DeliveryConfig>, all: Vec<MergedPullRequest>) -> Vec<u64> {
        of_team(delivery, all)
            .iter()
            .map(|one| one.number)
            .collect()
    }

    fn keel() -> DeliveryConfig {
        DeliveryConfig {
            repos: vec!["acme/app".into()],
            team_field: Some("Team".into()),
            team_value: Some("Keel".into()),
        }
    }

    #[test]
    fn without_a_team_field_every_pull_request_counts() {
        let all = vec![pull_request(1, &[]), pull_request(2, &[("Team", "Web")])];
        let plain = DeliveryConfig {
            team_field: None,
            team_value: None,
            ..keel()
        };
        assert_eq!(numbers(Some(&plain), all.clone()), [1, 2]);
        assert_eq!(numbers(None, all), [1, 2]);
    }

    #[test]
    fn with_one_only_the_pull_requests_whose_issue_has_the_teams_option_count() {
        let all = vec![
            pull_request(1, &[("Team", "Keel")]),
            pull_request(2, &[("Team", "Web")]),
            pull_request(3, &[("Status", "Done")]),
            pull_request(4, &[]),
            pull_request(5, &[("Status", "Keel")]),
            pull_request(6, &[(" team ", " keel ")]),
        ];
        assert_eq!(numbers(Some(&keel()), all), [1, 6]);
    }

    #[test]
    fn the_first_option_the_board_lists_decides_when_an_issue_has_two() {
        let all = vec![
            pull_request(1, &[("Team", "Web"), ("Team", "Keel")]),
            pull_request(2, &[("Team", "Keel"), ("Team", "Web")]),
        ];
        assert_eq!(numbers(Some(&keel()), all), [2]);
    }
}

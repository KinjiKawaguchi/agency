//! The Client API's table in docs/protocol.md names every route the node declares, and no other: the table
//! stays prose, and this test holds its methods and paths to the declarations.
use agency_adapters::client_api::routes_behind_the_guard;
use std::collections::BTreeSet;

const PROTOCOL: &str = include_str!("../../../docs/protocol.md");

/// The routes declared behind the guard that the table leaves to a section of their own.
const ELSEWHERE: [(&str, &str); 2] = [("GET", "/app/handoff"), ("GET", "/app/callback")];

/// `/api/orgs/<org>/usage?days=<n>` as the router has it: `/api/orgs/{org}/usage`.
fn as_declared(path: &str) -> String {
    let path = path.split('?').next().unwrap_or_default();
    path.replace('<', "{").replace('>', "}")
}

/// The paths of one row's path cell. A row may name a second path relative to the first one's parent, as
/// "`/api/orgs/<org>/evaluation?days=<n>` and `/effort?days=<n>`" does.
fn paths_of(cell: &str) -> Vec<String> {
    let named: Vec<&str> = cell.split('`').skip(1).step_by(2).collect();
    let Some((first, rest)) = named.split_first() else {
        return vec![];
    };
    let first = as_declared(first);
    let parent = &first[..first.rfind('/').unwrap_or(0)];
    let others = rest.iter().map(|path| match as_declared(path) {
        absolute if absolute.starts_with("/api/") || absolute.starts_with("/metrics") => absolute,
        relative => format!("{parent}{relative}"),
    });
    std::iter::once(first.clone()).chain(others).collect()
}

/// The `(method, path)` of each row of the table under "## Client API".
fn documented() -> BTreeSet<(String, String)> {
    let section = PROTOCOL.split("\n## Client API\n").nth(1).unwrap();
    let section = section.split("\n### ").next().unwrap();
    section
        .lines()
        .filter_map(|line| line.strip_prefix("| "))
        .filter_map(|row| {
            let mut cells = row.split(" | ");
            Some((cells.next()?, cells.next()?))
        })
        .filter(|(method, _)| matches!(*method, "GET" | "POST" | "DELETE" | "PUT" | "PATCH"))
        .flat_map(|(method, cell)| {
            paths_of(cell)
                .into_iter()
                .map(move |path| (method.to_owned(), path))
        })
        .collect()
}

#[test]
fn the_client_api_table_names_exactly_the_declared_routes() {
    let declared: BTreeSet<(String, String)> = routes_behind_the_guard()
        .into_iter()
        .filter(|route| !ELSEWHERE.contains(route))
        .map(|(method, path)| (method.to_owned(), path.to_owned()))
        .collect();
    let documented = documented();
    let missing: Vec<_> = declared.difference(&documented).collect();
    let unknown: Vec<_> = documented.difference(&declared).collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "docs/protocol.md's Client API table and the declared routes differ: not in the table {missing:?}; not declared {unknown:?}"
    );
}

#[test]
fn a_row_reads_as_the_router_has_it() {
    assert_eq!(
        paths_of("`/api/orgs/<org>/evaluation?days=<n>` and `/effort?days=<n>`"),
        ["/api/orgs/{org}/evaluation", "/api/orgs/{org}/effort"]
    );
    assert_eq!(
        paths_of("`/api/push/tokens/<token>`"),
        ["/api/push/tokens/{token}"]
    );
    assert!(documented().len() > 30, "the table is read");
}

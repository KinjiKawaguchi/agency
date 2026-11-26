//! What a merged pull request is, from its title: the Conventional Commits type, or a revert.
use agency_proto::evaluation::MergedCounts;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Feat,
    Fix,
    Refactor,
    Docs,
    Test,
    Chore,
    /// Any other title, including `ci`, `build`, `perf`, and `style`.
    Other,
    /// A pull request that reverts another. It is counted on its own, not as any type.
    Revert,
}

/// The type of a title such as `feat(web)!: add a screen`: the letters before an optional scope in
/// parentheses, an optional `!`, and a colon. Lower-cased.
fn conventional_type(title: &str) -> Option<String> {
    let (head, _) = title.split_once(':')?;
    let head = head.strip_suffix('!').unwrap_or(head);
    let name = match head.split_once('(') {
        Some((name, rest))
            if rest.ends_with(')') && !rest[..rest.len() - 1].contains(['(', ')']) =>
        {
            name
        }
        Some(_) => return None,
        None => head,
    };
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphabetic()))
        .then(|| name.to_ascii_lowercase())
}

/// A title that starts with the word `Revert` (GitHub's own title for a revert is `Revert "…"`).
fn reverts_by_title(title: &str) -> bool {
    let lower = title.trim_start().to_ascii_lowercase();
    lower.strip_prefix("revert").is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|next| !next.is_ascii_alphanumeric())
    })
}

/// What the pull request is. A revert is a title that starts with `Revert` or a body that says
/// "This reverts commit".
pub fn classify(title: &str, body: &str) -> Kind {
    if reverts_by_title(title) || body.contains("This reverts commit") {
        return Kind::Revert;
    }
    match conventional_type(title).as_deref() {
        Some("feat") => Kind::Feat,
        Some("fix") => Kind::Fix,
        Some("refactor") => Kind::Refactor,
        Some("docs") => Kind::Docs,
        Some("test") => Kind::Test,
        Some("chore") => Kind::Chore,
        _ => Kind::Other,
    }
}

/// Adds one merged pull request to the counts. A revert is not a type, so it is not added.
pub fn count(counts: &mut MergedCounts, kind: Kind) {
    let slot = match kind {
        Kind::Feat => &mut counts.feat,
        Kind::Fix => &mut counts.fix,
        Kind::Refactor => &mut counts.refactor,
        Kind::Docs => &mut counts.docs,
        Kind::Test => &mut counts.test,
        Kind::Chore => &mut counts.chore,
        Kind::Other => &mut counts.other,
        Kind::Revert => return,
    };
    *slot += 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_type_of_a_conventional_title() {
        let kind = |title| classify(title, "");
        assert_eq!(kind("feat: add a screen"), Kind::Feat);
        assert_eq!(kind("feat(web): add a screen"), Kind::Feat);
        assert_eq!(
            kind("fix(core)!: refuse it"),
            Kind::Fix,
            "a breaking change is still a fix"
        );
        assert_eq!(
            kind("refactor(web-client): x"),
            Kind::Refactor,
            "a scope may hold a dash"
        );
        assert_eq!(kind("refactor: x"), Kind::Refactor);
        assert_eq!(kind("docs: x"), Kind::Docs);
        assert_eq!(kind("test(proto): x"), Kind::Test);
        assert_eq!(kind("chore(deps): update x"), Kind::Chore);
        assert_eq!(kind("Feat: capitalised"), Kind::Feat);
    }

    #[test]
    fn anything_else_is_other() {
        let kind = |title| classify(title, "");
        for title in [
            "ci: x",
            "build: x",
            "perf: x",
            "style: x",
            "add a screen",
            "",
            ":",
            "feat add: x",
            "feat(: x",
            "feat(a)(b): x",
            "(web): x",
            "fe at: x",
        ] {
            assert_eq!(kind(title), Kind::Other, "{title:?}");
        }
    }

    #[test]
    fn a_revert_is_found_by_its_title_or_its_body() {
        assert_eq!(classify("Revert \"feat: add a screen\"", ""), Kind::Revert);
        assert_eq!(classify("revert: add a screen", ""), Kind::Revert);
        assert_eq!(classify("revert(web): add a screen", ""), Kind::Revert);
        assert_eq!(classify("  Revert", ""), Kind::Revert);
        assert_eq!(
            classify("fix: undo it", "This reverts commit 1234abc."),
            Kind::Revert
        );
        assert_eq!(
            classify("reverted: not one", ""),
            Kind::Other,
            "revert must be a word of its own"
        );
        assert_eq!(
            classify("fix: revert nothing", ""),
            Kind::Fix,
            "only a title that starts with it"
        );
        assert_eq!(classify("feat: x", "it never reverts commit"), Kind::Feat);
    }

    #[test]
    fn counts_each_type_and_leaves_reverts_out() {
        let mut counts = MergedCounts::default();
        for kind in [
            Kind::Feat,
            Kind::Feat,
            Kind::Fix,
            Kind::Refactor,
            Kind::Docs,
            Kind::Test,
            Kind::Chore,
            Kind::Other,
            Kind::Revert,
        ] {
            count(&mut counts, kind);
        }
        assert_eq!(
            counts,
            MergedCounts {
                feat: 2,
                fix: 1,
                refactor: 1,
                docs: 1,
                test: 1,
                chore: 1,
                other: 1
            }
        );
    }
}

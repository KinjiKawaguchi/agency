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

/// A body line that `git revert` or GitHub's Revert button writes: `This reverts commit <sha>` or
/// `Reverts <owner/name>#<number>`. A sentence that only mentions them does not start a line like that.
fn reverts_by_body(body: &str) -> bool {
    let is_sha = |word: &str| {
        let sha = word.trim_end_matches('.');
        (7..=40).contains(&sha.len()) && sha.chars().all(|c| c.is_ascii_hexdigit())
    };
    let is_reference = |word: &str| {
        let Some((repo, number)) = word.split_once('#') else {
            return false;
        };
        let number = number.trim_end_matches('.');
        let named = repo
            .split_once('/')
            .is_some_and(|(owner, name)| !owner.is_empty() && !name.is_empty());
        named && !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
    };
    body.lines().map(str::trim).any(|line| {
        if let Some(rest) = line.strip_prefix("This reverts commit ") {
            return rest.split_whitespace().next().is_some_and(is_sha);
        }
        line.strip_prefix("Reverts ")
            .is_some_and(|rest| rest.split_whitespace().next().is_some_and(is_reference))
    })
}

/// What the pull request is. A revert is a title that starts with `Revert`, or a body with a line that
/// says which commit or pull request it reverts.
pub fn classify(title: &str, body: &str) -> Kind {
    if reverts_by_title(title) || reverts_by_body(body) {
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
            classify(
                "fix: undo it",
                "Why\n\nThis reverts commit 1234abcdef0123456789."
            ),
            Kind::Revert,
            "a line of its own"
        );
        assert_eq!(
            classify("fix: undo it", "  Reverts acme/app#123"),
            Kind::Revert,
            "what GitHub's Revert button writes"
        );
        assert_eq!(
            classify("fix: undo it", "Reverts acme/app#123."),
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
        for prose in [
            "A body that says \"This reverts commit\" in a sentence about the rule.",
            "- A revert is a PR whose body says \"This reverts commit\"",
            "This reverts commit",
            "This reverts commit not-a-sha.",
            "This reverts commit abc",
            "Reverts the thing",
            "Reverts acme/app",
            "Reverts acme/app#",
            "Reverts #12",
            "Reverts acme/app#12x",
        ] {
            assert_eq!(classify("feat: x", prose), Kind::Feat, "{prose:?}");
        }
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

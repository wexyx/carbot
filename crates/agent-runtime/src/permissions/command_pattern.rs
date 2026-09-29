/// Glob matching over the complete command line, after rejecting shell composition.
/// Patterns are human-authored; a broad pattern deliberately allows broad arguments.
pub(super) fn valid_line(value: &str) -> bool {
    !value.is_empty()
        && !value
            .chars()
            .any(|c| c.is_control() || ";&|><$`\\[](){}".contains(c))
}
pub(super) fn valid_pattern(value: &str) -> bool {
    valid_line(value)
        && value
            .split_whitespace()
            .next()
            .is_some_and(|binary| !binary.contains(['*', '?']) && !binary.contains('='))
}
pub(super) fn matches(pattern: &str, command: &str) -> bool {
    let command = command.trim();
    if command.len() > 8192 || !valid_line(command) {
        return false;
    }
    if pattern.strip_suffix(" *") == Some(command) {
        return true;
    }
    let pattern = pattern.chars().collect::<Vec<_>>();
    let command = command.chars().collect::<Vec<_>>();
    let (mut p, mut c, mut star, mut retry) = (0, 0, None, 0);
    while c < command.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == command[c]) {
            p += 1;
            c += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            retry = c;
        } else if let Some(at) = star {
            retry += 1;
            c = retry;
            p = at + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wildcard_rules_are_anchored_and_cannot_approve_shell_composition() {
        for (rule, command) in [
            ("git status *", "git status"),
            ("git status *", "git status --short"),
            ("cat *.md", "cat README.md"),
            ("cat ?.md", "cat 文.md"),
        ] {
            assert!(matches(rule, command));
            assert!(valid_pattern(rule));
        }
        for command in [
            "cat README.md; id",
            "cat $(id).md",
            "cat a.md | sh",
            "cat a.md\nid",
            "cat a.md > out",
            "git status && rm file",
        ] {
            assert!(!matches("*", command));
        }
        assert!(!matches("git status *", "git reset --hard"));
        assert!(!matches("cat ?.md", "cat README.md"));
        assert!(!valid_pattern("*"));
        assert!(!valid_pattern("git* status"));
    }
}

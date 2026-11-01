//! The files that an asset directory leaves out of the site.
//!
//! A pattern is written like a line of a `.gitignore` file, in a smaller form:
//!
//! | Pattern        | Leaves out                                                       |
//! | -------------- | ---------------------------------------------------------------- |
//! | `*.dot`        | the files and directories of that name, at any depth             |
//! | `draft/`       | the directories of that name, at any depth, with what they hold  |
//! | `raw/*.png`    | the paths from the root of the asset directory                   |
//! | `**/old-*`     | the same, with `**` for any number of directories, none included |
//!
//! `*` stands for any run of characters and `?` for 1 character, neither of them a slash. A
//! pattern with no slash but a last one is told from the name alone.

/// Whether the path `rel`, from the root of an asset directory, is left out by one of the patterns.
/// A path is left out when it or one of the directories above it matches.
pub fn is_excluded(patterns: &[String], rel: &str) -> bool {
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    (1..=parts.len()).any(|n| {
        let is_dir = n < parts.len();
        let path = parts[..n].join("/");
        patterns.iter().any(|p| matches(p, &path, is_dir))
    })
}

/// Whether one of the patterns matches the path `rel`, a directory or not, as a walk of the
/// directory asks it of each entry.
pub fn matches_any(patterns: &[String], rel: &str, is_dir: bool) -> bool {
    patterns.iter().any(|p| matches(p, rel, is_dir))
}

/// Whether one pattern matches the path `rel`, a directory or not.
pub fn matches(pattern: &str, rel: &str, is_dir: bool) -> bool {
    let dir_only = pattern.ends_with('/');
    if dir_only && !is_dir {
        return false;
    }
    let pattern = pattern.trim_end_matches('/');
    if pattern.is_empty() {
        return false;
    }
    if !pattern.contains('/') {
        let name = rel.rsplit('/').next().unwrap_or(rel);
        return segment(pattern.as_bytes(), name.as_bytes());
    }
    let pattern: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
    let path: Vec<&str> = rel.split('/').collect();
    path_matches(&pattern, &path)
}

fn path_matches(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| path_matches(rest, &path[skip..])),
        Some((first, rest)) => match path.split_first() {
            Some((name, below)) => {
                segment(first.as_bytes(), name.as_bytes()) && path_matches(rest, below)
            }
            None => false,
        },
    }
}

/// A name against a pattern of `*` and `?`.
fn segment(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.split_first() {
        None => name.is_empty(),
        Some((b'*', rest)) => (0..=name.len()).any(|skip| segment(rest, &name[skip..])),
        Some((b'?', rest)) => !name.is_empty() && segment(rest, &name[1..]),
        Some((c, rest)) => name.first() == Some(c) && segment(rest, &name[1..]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn left_out(patterns: &[&str], rel: &str) -> bool {
        let patterns: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
        is_excluded(&patterns, rel)
    }

    #[test]
    fn a_name_without_a_slash_is_told_at_any_depth() {
        assert!(left_out(&["*.dot"], "a.dot"));
        assert!(left_out(&["*.dot"], "deep/er/b.dot"));
        assert!(!left_out(&["*.dot"], "a.svg"));
        assert!(left_out(&["features-*"], "x/features-compare.svg"));
    }

    #[test]
    fn a_directory_is_left_out_with_what_it_holds() {
        assert!(left_out(&["draft/"], "draft/a.png"));
        assert!(left_out(&["draft/"], "x/draft/y/a.png"));
        assert!(!left_out(&["draft/"], "draft"), "a file called draft stays");
        assert!(left_out(&["draft"], "draft"));
    }

    #[test]
    fn a_pattern_with_a_slash_starts_at_the_root() {
        assert!(left_out(&["raw/*.png"], "raw/a.png"));
        assert!(!left_out(&["raw/*.png"], "x/raw/a.png"));
        assert!(!left_out(&["raw/*.png"], "raw/sub/a.png"));
    }

    #[test]
    fn double_stars_stand_for_any_number_of_directories() {
        assert!(left_out(&["**/old-*"], "old-a.png"));
        assert!(left_out(&["**/old-*"], "x/y/old-a.png"));
        assert!(left_out(&["raw/**/*.tmp"], "raw/a.tmp"));
        assert!(left_out(&["raw/**/*.tmp"], "raw/a/b/c.tmp"));
        assert!(!left_out(&["raw/**/*.tmp"], "other/a.tmp"));
    }

    #[test]
    fn question_marks_and_stars_do_not_cross_a_slash() {
        assert!(left_out(&["a?.png"], "ab.png"));
        assert!(!left_out(&["a?.png"], "abc.png"));
        assert!(!left_out(&["x/*.png"], "x/y/a.png"));
        assert!(!left_out(&[], "anything"));
    }
}

//! The conversion of the real documentation of HAProxy, from a clone of its repository.
//!
//! The test is skipped unless `HAPROXY_REPO` names a clone. It converts the 3 documents of every
//! version from 1.6 and checks that no text is lost, that the headings are all there, and that the
//! result has no heading that repeats an anchor.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use dkt::haproxy::convert;

/// The documents that are read in every version.
const DOCS: [&str; 3] = ["intro", "configuration", "management"];

struct Version {
    name: String,
    reference: String,
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn numbers(version: &str) -> Vec<u32> {
    version.split('.').filter_map(|n| n.parse().ok()).collect()
}

/// Each version from `from` at its last release.
fn versions(repo: &Path, from: &str) -> Vec<Version> {
    let tags = git(repo, &["tag", "--list", "v*"]).expect("the tags of the repository");
    let mut last: std::collections::BTreeMap<Vec<u32>, (Vec<u32>, String)> = Default::default();
    for tag in tags.split_whitespace() {
        let n = numbers(tag.trim_start_matches('v'));
        if n.len() != 3 || tag.contains(['-', '_']) || n[..2] < numbers(from)[..] {
            continue;
        }
        let entry = last
            .entry(n[..2].to_vec())
            .or_insert_with(|| (n.clone(), tag.into()));
        if n > entry.0 {
            *entry = (n, tag.into());
        }
    }
    last.into_iter()
        .map(|(b, (_, tag))| Version {
            name: format!("{}.{}", b[0], b[1]),
            reference: tag,
        })
        .collect()
}

fn source(repo: &Path, version: &Version, name: &str) -> Option<String> {
    git(
        repo,
        &["show", &format!("{}:doc/{name}.txt", version.reference)],
    )
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty() && !w.bytes().all(|b| b.is_ascii_digit()))
        .map(|w| w.to_ascii_lowercase())
        // The labels that the conversion turns into alerts and into one form of `Example`.
        .filter(|w| !["note", "notes", "important", "warning", "attention"].contains(&w.as_str()))
        .map(|w| {
            if w == "examples" {
                "example".to_string()
            } else {
                w
            }
        })
        .collect()
}

/// The shingles of 5 words of a text.
fn shingles(words: &[String]) -> HashSet<Vec<&str>> {
    words
        .windows(5)
        .map(|w| w.iter().map(String::as_str).collect())
        .collect()
}

/// The text of Markdown without the targets of its links, which add words to a sentence.
fn without_targets(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut rest = markdown;
    while let Some(at) = rest.find("](") {
        out.push_str(&rest[..at + 1]);
        let tail = &rest[at + 2..];
        match tail.find(')') {
            Some(close) => rest = &tail[close + 1..],
            None => {
                rest = tail;
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The source without the lines that the conversion rewrites on purpose.
fn comparable(source: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for line in source.lines() {
        let t = line.trim_start();
        if skip {
            skip = false;
            continue;
        }
        if t.starts_with("May be used in sections") || t.starts_with("Usable in") {
            skip = true;
            continue;
        }
        if t.starts_with("May be used in the following contexts") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

#[test]
fn the_documentation_of_every_version_keeps_its_text_and_its_headings() {
    let Ok(repo) = std::env::var("HAPROXY_REPO") else {
        eprintln!("HAPROXY_REPO is not set: the documentation of HAProxy is not checked");
        return;
    };
    let repo = PathBuf::from(repo);
    let versions = versions(&repo, "1.6");
    assert!(!versions.is_empty());
    for version in &versions {
        for name in DOCS {
            let Some(source) = source(&repo, version, name) else {
                continue;
            };
            let out = convert(&source);
            let label = format!("{} {name}", version.name);

            let before = words(&comparable(&source));
            let after_words = words(&without_targets(&out.markdown));
            let kept = shingles(&after_words);
            let missing = before
                .windows(5)
                .filter(|w| !kept.contains(&w.iter().map(String::as_str).collect::<Vec<_>>()))
                .count();
            let total = before.len().saturating_sub(4).max(1);
            assert!(
                missing * 100 <= total * 3,
                "{label}: {missing} of {total} groups of 5 words are lost"
            );

            let numbered = source
                .lines()
                .zip(source.lines().skip(1))
                .filter(|(a, b)| {
                    let a = a.trim_end();
                    !a.starts_with(' ')
                        && a.starts_with(|c: char| c.is_ascii_digit())
                        && b.len() >= 3
                        && b.bytes().all(|c| c == b'-')
                })
                .count();
            let mut in_fence = false;
            let mut headings = 0;
            let mut seen = HashSet::new();
            for line in out.markdown.lines() {
                if line.starts_with("```") {
                    in_fence = !in_fence;
                }
                if in_fence || !line.starts_with('#') {
                    continue;
                }
                if line
                    .trim_start_matches('#')
                    .trim_start()
                    .starts_with(|c: char| c.is_ascii_digit())
                {
                    headings += 1;
                }
                assert!(
                    seen.insert(line.to_string()) || line.starts_with("# "),
                    "{label}: the heading {line:?} repeats"
                );
            }
            assert_eq!(numbered, headings, "{label}: a chapter is missing");
        }
    }
}

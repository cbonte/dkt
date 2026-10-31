//! A search index for a built site: the title of every page and of every section of every page.
//!
//! The index is one JSON file at the root of the site. A theme reads it in the browser and matches
//! what the reader types against the titles.

use std::collections::HashSet;

use crate::page::{Alias, Heading};

/// Name of the index file at the root of the output directory.
pub const FILE: &str = "search.json";

/// The deepest heading level that is indexed.
const DEEPEST: u8 = 4;

/// A title the reader can reach.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// The title of the page or of the section.
    pub title: String,
    /// The title of the page that holds it.
    pub page: String,
    /// Where it leads: the address of the page, with the anchor of the section.
    pub url: String,
    /// 1 for a page, 2 and more for a section.
    pub level: u8,
    /// The title of the section above, for a section under another section.
    pub parent: String,
    /// The description of a page.
    pub description: String,
    /// Words that find it besides its title.
    pub aliases: String,
}

/// What the index needs to know of a page.
pub struct Source<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub url: &'a str,
    pub headings: &'a [Heading],
    pub aliases: &'a [Alias],
}

/// The words given to the heading with this id, or to the page when `id` is its first level one
/// heading or when the alias was written above every heading.
fn words(aliases: &[Alias], matches: impl Fn(Option<&str>) -> bool) -> String {
    let found: Vec<&str> = aliases
        .iter()
        .filter(|a| matches(a.anchor.as_deref()))
        .map(|a| a.words.as_str())
        .collect();
    found.join(" ")
}

/// The entries of a set of pages. A page comes first, then its sections in the order of the
/// document.
pub fn entries<'a>(pages: impl IntoIterator<Item = Source<'a>>) -> Vec<Entry> {
    let mut out = Vec::new();
    for page in pages {
        let top = page
            .headings
            .iter()
            .find(|h| h.level == 1)
            .map(|h| h.id.as_str());
        out.push(Entry {
            title: page.title.to_string(),
            page: page.title.to_string(),
            url: page.url.to_string(),
            level: 1,
            description: page.description.to_string(),
            aliases: words(page.aliases, |anchor| anchor.is_none() || anchor == top),
            ..Entry::default()
        });
        let mut above: Vec<&Heading> = Vec::new();
        for h in page
            .headings
            .iter()
            .filter(|h| (2..=DEEPEST).contains(&h.level) && !h.id.is_empty())
        {
            while above.last().is_some_and(|p| p.level >= h.level) {
                above.pop();
            }
            out.push(Entry {
                title: h.text.clone(),
                page: page.title.to_string(),
                url: format!("{}#{}", page.url, h.id),
                level: h.level,
                parent: above.last().map(|p| p.text.clone()).unwrap_or_default(),
                aliases: words(page.aliases, |anchor| anchor == Some(h.id.as_str())),
                ..Entry::default()
            });
            above.push(h);
        }
    }
    out
}

fn quote(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The index as JSON: a list of objects with `t` the title, `p` the page, `u` the address and `l`
/// the level, and when they are not empty `b` the section above, `d` the description and `a` the
/// aliases.
pub fn to_json(entries: &[Entry]) -> String {
    let mut out = String::from("[");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"t\":");
        quote(&e.title, &mut out);
        out.push_str(",\"p\":");
        quote(&e.page, &mut out);
        out.push_str(",\"u\":");
        quote(&e.url, &mut out);
        out.push_str(&format!(",\"l\":{}", e.level));
        for (key, value) in [("b", &e.parent), ("d", &e.description), ("a", &e.aliases)] {
            if !value.is_empty() {
                out.push_str(&format!(",\"{key}\":"));
                quote(value, &mut out);
            }
        }
        out.push('}');
    }
    out.push(']');
    out
}

/// The index of a partial build: what the existing index holds for the pages that were not built,
/// then the new entries. The existing index is read as the objects it is made of, and a page is
/// told by the address before the anchor of an entry.
pub fn merge(existing: &str, built: &HashSet<String>, new: &[Entry]) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let (mut start, mut depth, mut in_string, mut escaped) = (0, 0, false, false);
    for (at, c) in existing.char_indices() {
        if in_string {
            match (escaped, c) {
                (true, _) => escaped = false,
                (false, '\\') => escaped = true,
                (false, '"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    start = at;
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    kept.push(&existing[start..=at]);
                }
            }
            _ => {}
        }
    }
    let url_of = |object: &str| -> Option<String> {
        let after = &object[object.find(",\"u\":\"")? + 6..];
        let end = after.find('"')?;
        Some(
            after[..end]
                .split('#')
                .next()
                .unwrap_or_default()
                .to_string(),
        )
    };
    let mut out = String::from("[");
    let mut first = true;
    for object in kept {
        if url_of(object).is_some_and(|u| built.contains(&u)) {
            continue;
        }
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(object);
    }
    let added = to_json(new);
    let inner = added.trim_start_matches('[').trim_end_matches(']');
    if !inner.is_empty() {
        if !first {
            out.push(',');
        }
        out.push_str(inner);
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heading(level: u8, id: &str, text: &str) -> Heading {
        Heading {
            level,
            id: id.into(),
            text: text.into(),
        }
    }

    fn page<'a>(headings: &'a [Heading], aliases: &'a [Alias]) -> Source<'a> {
        Source {
            title: "dkt site",
            description: "Builds a site.",
            url: "/c/site/",
            headings,
            aliases,
        }
    }

    #[test]
    fn a_partial_build_keeps_the_other_pages_and_replaces_the_ones_it_built() {
        let old = "[{\"t\":\"A\",\"p\":\"A\",\"u\":\"/a/\",\"l\":1},{\"t\":\"S \\\"q\\\"\",\"p\":\"A\",\"u\":\"/a/#s\",\"l\":2},{\"t\":\"B\",\"p\":\"B\",\"u\":\"/b/\",\"l\":1}]";
        let built: HashSet<String> = ["/a/".to_string()].into();
        let new = [Entry {
            title: "New".into(),
            page: "New".into(),
            url: "/a/".into(),
            level: 1,
            ..Entry::default()
        }];
        let merged = merge(old, &built, &new);
        assert_eq!(
            merged,
            "[{\"t\":\"B\",\"p\":\"B\",\"u\":\"/b/\",\"l\":1},{\"t\":\"New\",\"p\":\"New\",\"u\":\"/a/\",\"l\":1}]"
        );
        assert_eq!(merge("", &built, &[]), "[]");
    }

    #[test]
    fn a_page_comes_first_and_its_sections_follow_with_their_anchor() {
        let headings = [
            heading(1, "dkt-site", "dkt site"),
            heading(2, "theme", "Theme"),
            heading(3, "x", "Deep"),
            heading(5, "z", "Too deep"),
        ];
        let list = entries([page(&headings, &[])]);
        let urls: Vec<_> = list.iter().map(|e| e.url.as_str()).collect();
        assert_eq!(urls, ["/c/site/", "/c/site/#theme", "/c/site/#x"]);
        assert_eq!(list[0].level, 1);
        assert_eq!(list[0].description, "Builds a site.");
        assert_eq!(list[1].page, "dkt site");
    }

    #[test]
    fn a_section_knows_the_section_above_it() {
        let headings = [
            heading(2, "a", "A"),
            heading(3, "a1", "A one"),
            heading(3, "a2", "A two"),
            heading(2, "b", "B"),
            heading(3, "b1", "B one"),
        ];
        let list = entries([page(&headings, &[])]);
        let parents: Vec<_> = list.iter().map(|e| e.parent.as_str()).collect();
        assert_eq!(parents, ["", "", "A", "A", "", "B"]);
    }

    #[test]
    fn aliases_go_to_the_section_they_were_written_under_or_to_the_page() {
        let headings = [heading(1, "top", "dkt site"), heading(2, "theme", "Theme")];
        let aliases = [
            Alias {
                anchor: None,
                words: "above".into(),
            },
            Alias {
                anchor: Some("top".into()),
                words: "under-title".into(),
            },
            Alias {
                anchor: Some("theme".into()),
                words: "skin look".into(),
            },
        ];
        let list = entries([page(&headings, &aliases)]);
        assert_eq!(list[0].aliases, "above under-title");
        assert_eq!(list[1].aliases, "skin look");
    }

    #[test]
    fn a_heading_with_no_anchor_is_left_out() {
        let headings = [heading(2, "", "Nameless")];
        assert_eq!(entries([page(&headings, &[])]).len(), 1);
    }

    #[test]
    fn the_json_escapes_what_json_needs_and_leaves_empty_fields_out() {
        let list = vec![Entry {
            title: "a \"b\" \\ c\n".into(),
            page: "é".into(),
            url: "/u/".into(),
            level: 2,
            ..Entry::default()
        }];
        assert_eq!(
            to_json(&list),
            "[{\"t\":\"a \\\"b\\\" \\\\ c\\n\",\"p\":\"é\",\"u\":\"/u/\",\"l\":2}]"
        );
        assert_eq!(to_json(&[]), "[]");
        let full = vec![Entry {
            title: "t".into(),
            page: "p".into(),
            url: "/u/".into(),
            level: 3,
            parent: "up".into(),
            description: "d".into(),
            aliases: "x y".into(),
        }];
        assert_eq!(
            to_json(&full),
            "[{\"t\":\"t\",\"p\":\"p\",\"u\":\"/u/\",\"l\":3,\"b\":\"up\",\"d\":\"d\",\"a\":\"x y\"}]"
        );
    }
}

//! Where a link of a document goes once the documents are pages of a site.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Normalizes a relative path, `None` when it climbs out of the root.
pub fn normalize(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// The documents the site serves and its asset directories.
pub struct LinkMap {
    root: PathBuf,
    served: HashMap<String, String>,
    assets: Vec<Served>,
}

/// An asset directory: where it is in the tree, where it is served and what it leaves out.
pub struct Served {
    pub src: String,
    pub url: String,
    pub exclude: Vec<String>,
}

impl Served {
    /// The path inside the directory, when `rel` is a file that the directory serves.
    fn serves<'a>(&self, rel: &'a str) -> Option<&'a str> {
        let inside = rel.strip_prefix(&format!("{}/", self.src))?;
        (!crate::site::exclude::is_excluded(&self.exclude, inside)).then_some(inside)
    }
}

impl LinkMap {
    pub fn new(root: &Path, served: HashMap<String, String>, assets: Vec<Served>) -> LinkMap {
        LinkMap {
            root: root.to_path_buf(),
            served,
            assets,
        }
    }

    /// The path of `target`, written in the document `from`, from the root of the documents, and
    /// its anchor. `None` for an address, an anchor in the same document and a path that climbs
    /// out of the root.
    fn locate(&self, from: &str, target: &str) -> Option<(String, String)> {
        if target.starts_with('#') || target.starts_with('/') || target.contains("://") {
            return None;
        }
        if target.starts_with("mailto:") || target.starts_with("tel:") {
            return None;
        }
        let (path, frag) = match target.split_once('#') {
            Some((p, f)) => (p, format!("#{f}")),
            None => (target, String::new()),
        };
        if path.is_empty() {
            return None;
        }
        let dir = from.rsplit_once('/').map_or("", |(d, _)| d);
        Some((normalize(&format!("{dir}/{path}"))?, frag))
    }

    /// Where `target`, written in the document `from`, goes on the site. `None` leaves it as
    /// written: an address, an anchor in the same document, a target that the site does not serve.
    pub fn resolve(&self, from: &str, target: &str) -> Option<String> {
        let (rel, frag) = self.locate(from, target)?;
        if let Some(url) = self.served.get(&rel) {
            return Some(format!("{url}{frag}"));
        }
        for asset in &self.assets {
            if let Some(rest) = asset.serves(&rel) {
                return Some(format!("{}/{rest}{frag}", asset.url.trim_end_matches('/')));
            }
        }
        None
    }

    /// The path of `target` when it exists but the site serves neither it nor a directory that
    /// holds it, so that a link to it would leave the site.
    pub fn unserved(&self, from: &str, target: &str) -> Option<String> {
        let (rel, _) = self.locate(from, target)?;
        if self.served.contains_key(&rel) || self.assets.iter().any(|a| a.serves(&rel).is_some()) {
            return None;
        }
        self.root.join(&rel).exists().then_some(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> LinkMap {
        let mut served = HashMap::new();
        served.insert(
            "docs/guides/using.md".to_string(),
            "/s/guides/using/".to_string(),
        );
        served.insert("README.md".to_string(), "/s/tour/".to_string());
        LinkMap::new(
            Path::new("/nonexistent"),
            served,
            vec![Served {
                src: "docs/img".to_string(),
                url: "/s/img".to_string(),
                exclude: Vec::new(),
            }],
        )
    }

    #[test]
    fn a_served_document_becomes_its_page() {
        let m = map();
        assert_eq!(
            m.resolve("docs/guides/models.md", "using.md#start")
                .as_deref(),
            Some("/s/guides/using/#start")
        );
        assert_eq!(
            m.resolve("docs/guides/x.md", "../../README.md").as_deref(),
            Some("/s/tour/")
        );
    }

    #[test]
    fn an_asset_directory_is_served_under_its_address() {
        assert_eq!(
            map().resolve("docs/guides/x.md", "../img/a.png").as_deref(),
            Some("/s/img/a.png")
        );
    }

    #[test]
    fn what_is_left_alone() {
        let m = map();
        for t in [
            "#here",
            "https://a.b/c",
            "/abs",
            "mailto:a@b.c",
            "missing.md",
            "../../../out.md",
        ] {
            assert_eq!(m.resolve("docs/guides/x.md", t), None, "{t}");
        }
    }

    #[test]
    fn a_file_that_exists_and_is_not_served_is_unserved() {
        let dir = std::env::temp_dir().join(format!("dkt-links-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "").unwrap();
        std::fs::write(dir.join("README.md"), "").unwrap();
        let mut served = HashMap::new();
        served.insert("README.md".to_string(), "/s/".to_string());
        let m = LinkMap::new(
            &dir,
            served,
            vec![Served {
                src: "img".to_string(),
                url: "/s/img".to_string(),
                exclude: vec!["*.tmp".to_string()],
            }],
        );
        std::fs::create_dir_all(dir.join("img")).unwrap();
        std::fs::write(dir.join("img/a.tmp"), "").unwrap();
        assert_eq!(
            m.unserved("docs/a.md", "../img/a.tmp").as_deref(),
            Some("img/a.tmp"),
            "a file the directory leaves out is not served"
        );
        assert_eq!(
            m.unserved("docs/a.md", "../src/main.rs#L3").as_deref(),
            Some("src/main.rs")
        );
        assert_eq!(m.unserved("docs/a.md", "../README.md"), None);
        assert_eq!(m.unserved("docs/a.md", "../missing.rs"), None);
        assert_eq!(m.unserved("docs/a.md", "https://a.b/c"), None);
        assert_eq!(m.unserved("docs/a.md", "#here"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

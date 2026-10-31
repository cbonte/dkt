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

/// The documents the site serves, its asset directories and the repository for the rest.
pub struct LinkMap {
    root: PathBuf,
    served: HashMap<String, String>,
    assets: Vec<(String, String)>,
    blob: String,
    tree: String,
}

impl LinkMap {
    pub fn new(
        root: &Path,
        served: HashMap<String, String>,
        assets: Vec<(String, String)>,
        repo: &str,
        branch: &str,
    ) -> LinkMap {
        let repo = repo.trim_end_matches('/');
        LinkMap {
            root: root.to_path_buf(),
            served,
            assets,
            blob: format!("{repo}/blob/{branch}/"),
            tree: format!("{repo}/tree/{branch}/"),
        }
    }

    /// Where `target`, written in the document `from`, goes on the site. `None` leaves it as written:
    /// an address, an anchor in the same document, a target that does not exist.
    pub fn resolve(&self, from: &str, target: &str) -> Option<String> {
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
        let rel = normalize(&format!("{dir}/{path}"))?;
        if let Some(url) = self.served.get(&rel) {
            return Some(format!("{url}{frag}"));
        }
        for (src, url) in &self.assets {
            if let Some(rest) = rel.strip_prefix(&format!("{src}/")) {
                return Some(format!("{}/{rest}{frag}", url.trim_end_matches('/')));
            }
        }
        let on_disk = self.root.join(&rel);
        if on_disk.is_dir() {
            return Some(format!("{}{rel}{frag}", self.tree));
        }
        on_disk
            .exists()
            .then(|| format!("{}{rel}{frag}", self.blob))
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
            vec![("docs/img".to_string(), "/s/img".to_string())],
            "https://example.com/r/",
            "main",
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
}

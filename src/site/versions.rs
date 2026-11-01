//! Several versions of a documentation under one root.
//!
//! A version build writes `OUT/NAME/` with the base `BASE/NAME`.
//! It never writes a file of the root.
//!
//! The refresh of the root writes `versions.json`, `index.html` and `latest/` from the declared list.
//! It reads the version directories and changes none of them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use super::MARKER;
use super::config::{Config, Status};
use crate::escape;

/// The list of the versions, at the root.
pub const LIST: &str = "versions.json";
/// The pages of a version and their anchors, in the directory of the version.
pub const PAGES: &str = "version.json";
/// The directory of the redirect pages to the latest release.
pub const ALIAS: &str = "latest";
/// The root page.
const ROOT_PAGE: &str = "index.html";
/// The format of `versions.json` and `version.json`.
/// A newer format adds fields and never renames one.
const FORMAT: u32 = 1;
/// The version of dkt, written in the manifest of a version directory.
const DKT: &str = env!("CARGO_PKG_VERSION");

/// Refuses a name that is not a single safe path segment, or that names a file of the root.
pub fn check_name(name: &str) -> Result<()> {
    let segment = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c));
    if !segment {
        bail!(
            "'{name}' is not a version name: use letters, digits, '.', '_' and '-', and do not start with '.'"
        );
    }
    if [ALIAS, LIST, ROOT_PAGE].contains(&name) {
        bail!("'{name}' names a file of the root and cannot name a version");
    }
    Ok(())
}

/// A version in `versions.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Listed {
    pub name: String,
    pub title: String,
    pub status: Status,
    pub url: String,
}

/// The content of `versions.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    pub format: u32,
    /// The name of the latest release, absent when no version is released.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    pub versions: Vec<Listed>,
}

/// The content of `version.json`: each page by its path inside the version, with its anchors.
#[derive(Debug, Serialize, Deserialize)]
pub struct Pages {
    pub format: u32,
    pub name: String,
    pub pages: BTreeMap<String, Vec<String>>,
}

/// What `.dkt-site` of a version directory says.
#[derive(Debug, Default, PartialEq)]
pub struct Manifest {
    /// The version of dkt that wrote the directory, absent in an empty marker.
    pub dkt: Option<String>,
    /// The files the build wrote, absent in an empty marker: every file then came from dkt.
    pub files: Option<BTreeSet<String>>,
}

impl Manifest {
    pub fn parse(text: &str) -> Manifest {
        if text.trim().is_empty() {
            return Manifest::default();
        }
        let mut manifest = Manifest {
            dkt: None,
            files: Some(BTreeSet::new()),
        };
        for line in text.lines() {
            if let Some(version) = line.strip_prefix("dkt ") {
                manifest.dkt = Some(version.to_string());
            } else if let Some(file) = line.strip_prefix("file ")
                && let Some(files) = manifest.files.as_mut()
            {
                files.insert(file.to_string());
            }
        }
        manifest
    }

    /// The text of the manifest of a build of `name` that wrote `files`.
    pub fn text(name: &str, files: &BTreeSet<String>) -> String {
        let mut out = format!("dkt {DKT}\nversion {name}\n");
        for file in files {
            out.push_str(&format!("file {file}\n"));
        }
        out
    }
}

/// What a template reads as `site.version`.
#[derive(Debug, Serialize)]
pub struct VersionView {
    pub name: String,
    pub title: String,
    pub status: Status,
    /// Whether the version is the latest release.
    pub latest: bool,
    /// The address of the set of versions, without a trailing slash.
    pub root: String,
    /// The warning the page carries: `dev`, `deprecated`, `newer`, or empty for none.
    pub note: &'static str,
}

/// An entry of the per page `versions` of a template.
#[derive(Debug, Serialize)]
pub struct VersionLink {
    pub name: String,
    pub title: String,
    pub status: Status,
    /// The same page in that version when its directory holds it, the home of that version otherwise.
    pub url: String,
    pub current: bool,
    pub latest: bool,
}

/// The path of the address part of `url`, without a trailing slash.
fn path_of(url: &str) -> String {
    super::base_path(url)
}

/// The scheme and the host of `url`, empty for an address without them.
fn origin_of(url: &str) -> &str {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let host = rest.find('/').map_or(rest, |at| &rest[..at]);
            &url[..scheme.len() + 3 + host.len()]
        }
        None => "",
    }
}

/// The label of a version in a list, with its state written as text.
pub fn label(title: &str, status: Status, latest: bool) -> String {
    let state = if latest {
        " (latest)"
    } else {
        match status {
            Status::Dev => " (development)",
            Status::Deprecated => " (deprecated)",
            Status::Release => "",
        }
    };
    format!("{title}{state}")
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .with_context(|| format!("cannot read {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
}

/// The files under `dir`, as paths from it with `/` between the parts.
fn files_under(dir: &Path) -> Result<BTreeSet<String>> {
    fn walk(dir: &Path, rel: &str, out: &mut BTreeSet<String>) -> Result<()> {
        for entry in fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name
            } else {
                format!("{rel}/{name}")
            };
            if entry.file_type()?.is_dir() {
                walk(&entry.path(), &path, out)?;
            } else {
                out.insert(path);
            }
        }
        Ok(())
    }
    let mut out = BTreeSet::new();
    if dir.is_dir() {
        walk(dir, "", &mut out)?;
    }
    Ok(out)
}

fn remove_dir_if_any(dir: &Path) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir).with_context(|| format!("cannot remove {}", dir.display()))?;
    }
    Ok(())
}

/// Puts `new` in place of `dir` with 2 renames, then removes the old directory.
/// `old` holds the previous directory between the 2 renames.
fn swap(new: &Path, dir: &Path, old: &Path) -> Result<()> {
    remove_dir_if_any(old)?;
    if dir.exists() {
        fs::rename(dir, old).with_context(|| format!("cannot move {}", dir.display()))?;
    }
    fs::rename(new, dir).with_context(|| format!("cannot move {}", new.display()))?;
    remove_dir_if_any(old)
}

/// Writes `text` to a sibling of `path`, then renames it over `path`.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("a file of the root has no name")?;
    let tmp = path.with_file_name(format!(".{name}.dkt-new"));
    fs::write(&tmp, text).with_context(|| format!("cannot write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("cannot write {}", path.display()))
}

/// The sibling of a version directory that a build writes into, and the one that holds the old
/// directory during the swap.
fn siblings(root: &Path, name: &str) -> (PathBuf, PathBuf) {
    (
        root.join(format!(".{name}.dkt-new")),
        root.join(format!(".{name}.dkt-old")),
    )
}

/// The build of one version into its directory under the root.
///
/// The pages go to a sibling directory, which replaces the version directory once the build ends.
/// A build that stops before the end leaves the version directory as it was.
pub struct Build {
    root: PathBuf,
    name: String,
    dir: PathBuf,
    staging: PathBuf,
    old: PathBuf,
    /// The address of the root, as given to the build.
    root_url: String,
    /// The path of the address of the root.
    pub root_path: String,
    versions: Vec<Listed>,
    latest: Option<String>,
    started: bool,
    done: bool,
}

impl Build {
    /// Checks the name and the output tree, and reads the published list of the versions.
    pub fn prepare(root: &Path, name: &str, root_url: &str) -> Result<Build> {
        check_name(name)?;
        if root.join(MARKER).exists() {
            bail!(
                "{} holds a site built without versions, so a version cannot be built into it",
                root.display()
            );
        }
        let dir = root.join(name);
        if dir.exists() {
            let empty = fs::read_dir(&dir)?.next().is_none();
            if !empty && !dir.join(MARKER).exists() {
                bail!(
                    "{} is not empty and was not made by dkt site, so it is not replaced",
                    dir.display()
                );
            }
        }
        let root_path = path_of(root_url);
        let published: Option<List> = read_json(&root.join(LIST))?;
        let latest = published.as_ref().and_then(|l| l.latest.clone());
        let mut versions: Vec<Listed> = published
            .map(|l| l.versions)
            .unwrap_or_default()
            .into_iter()
            .filter(|v| check_name(&v.name).is_ok())
            .map(|v| Listed {
                url: format!("{root_path}/{}/", v.name),
                ..v
            })
            .collect();
        if !versions.iter().any(|v| v.name == name) {
            eprintln!(
                "dkt site: warning: {} does not list the version {name}: declare it, then run dkt site --only versions",
                root.join(LIST).display()
            );
            versions.insert(
                0,
                Listed {
                    name: name.to_string(),
                    title: name.to_string(),
                    status: Status::Release,
                    url: format!("{root_path}/{name}/"),
                },
            );
        }
        let (staging, old) = siblings(root, name);
        Ok(Build {
            root: root.to_path_buf(),
            name: name.to_string(),
            dir,
            staging,
            old,
            root_url: root_url.trim_end_matches('/').to_string(),
            root_path,
            versions,
            latest,
            started: false,
            done: false,
        })
    }

    /// The address of the version, which is the base of its pages.
    pub fn base_url(&self) -> String {
        format!("{}/{}", self.root_url, self.name)
    }

    /// The directory the pages are written to until the swap.
    pub fn out(&self) -> &Path {
        &self.staging
    }

    /// The directory of the version.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    fn current(&self) -> &Listed {
        self.versions
            .iter()
            .find(|v| v.name == self.name)
            .expect("the version being built is listed")
    }

    /// What templates read as `site.version`.
    pub fn view(&self) -> VersionView {
        let current = self.current();
        let latest = self.latest.as_deref() == Some(&self.name);
        let note = match current.status {
            Status::Dev => "dev",
            Status::Deprecated => "deprecated",
            Status::Release if self.latest.is_some() && !latest => "newer",
            Status::Release => "",
        };
        VersionView {
            name: self.name.clone(),
            title: current.title.clone(),
            status: current.status,
            latest,
            root: self.root_path.clone(),
            note,
        }
    }

    /// The `versions` of the page at `rel`, its path inside the version such as `guide/x/`.
    pub fn links(&self, rel: &str) -> Vec<VersionLink> {
        self.versions
            .iter()
            .map(|v| {
                let current = v.name == self.name;
                let holds = current
                    || self
                        .root
                        .join(&v.name)
                        .join(rel)
                        .join("index.html")
                        .is_file();
                VersionLink {
                    name: v.name.clone(),
                    title: v.title.clone(),
                    status: v.status,
                    url: if holds {
                        format!("{}{rel}", v.url)
                    } else {
                        v.url.clone()
                    },
                    current,
                    latest: self.latest.as_deref() == Some(&v.name),
                }
            })
            .collect()
    }

    /// Empties the sibling directory left by an earlier build and makes it again.
    pub fn start(&mut self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("cannot create {}", self.root.display()))?;
        remove_dir_if_any(&self.staging)?;
        remove_dir_if_any(&self.old)?;
        self.started = true;
        fs::create_dir_all(&self.staging)
            .with_context(|| format!("cannot create {}", self.staging.display()))
    }

    /// Writes the pages of the version and their anchors, keeps the files of the old directory that
    /// dkt did not write, writes the manifest and puts the new directory in place.
    pub fn finish(mut self, pages: BTreeMap<String, Vec<String>>) -> Result<()> {
        let list = Pages {
            format: FORMAT,
            name: self.name.clone(),
            pages,
        };
        fs::write(
            self.staging.join(PAGES),
            serde_json::to_string(&list).context("cannot write the pages of the version")?,
        )?;
        let written = files_under(&self.staging)?;
        let previous = match fs::read_to_string(self.dir.join(MARKER)) {
            Ok(text) => Some(Manifest::parse(&text)),
            Err(_) => None,
        };
        if let Some(previous) = &previous {
            match &previous.dkt {
                Some(dkt) if dkt != DKT => eprintln!(
                    "dkt site: warning: {} was built by dkt {dkt}, and dkt {DKT} builds it now",
                    self.dir.display()
                ),
                None => eprintln!(
                    "dkt site: warning: {} was built by a dkt that does not record its version, and dkt {DKT} builds it now",
                    self.dir.display()
                ),
                _ => {}
            }
            // A file the old manifest does not list did not come from dkt, and moves to the new
            // directory as it is.
            if let Some(listed) = &previous.files {
                for file in files_under(&self.dir)? {
                    if file == MARKER || listed.contains(&file) {
                        continue;
                    }
                    if written.contains(&file) {
                        eprintln!(
                            "dkt site: warning: {} was not written by dkt, and the new build replaces it",
                            self.dir.join(&file).display()
                        );
                        continue;
                    }
                    let to = self.staging.join(&file);
                    if let Some(parent) = to.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(self.dir.join(&file), &to)
                        .with_context(|| format!("cannot keep {}", file))?;
                }
            }
        }
        fs::write(
            self.staging.join(MARKER),
            Manifest::text(&self.name, &written),
        )?;
        swap(&self.staging, &self.dir, &self.old)?;
        self.done = true;
        Ok(())
    }
}

impl Drop for Build {
    fn drop(&mut self) {
        if self.started && !self.done {
            let _ = fs::remove_dir_all(&self.staging);
        }
    }
}

/// The page with the meta that names the list of the versions, and the style and the script of
/// the selector, each added once.
pub fn with_versions(html: &str, base: &str, root: &str, name: &str) -> String {
    let (base, root, name) = (escape::html(base), escape::html(root), escape::html(name));
    let mut head = String::new();
    if !html.contains("name=\"dkt-versions\"") {
        head.push_str(&format!(
            "<meta name=\"dkt-versions\" content=\"{root}/{LIST}\" data-root=\"{root}\" data-current=\"{name}\">\n"
        ));
    }
    if !html.contains("/versions.css\"") {
        head.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"{base}/versions.css\">\n"
        ));
    }
    let mut out = html.to_string();
    if !head.is_empty() {
        let at = out.find("</head>").unwrap_or(0);
        out.insert_str(at, &head);
    }
    if !html.contains("/versions.js\"") {
        let script = format!("<script src=\"{base}/versions.js\" defer></script>\n");
        match out.rfind("</body>") {
            Some(at) => out.insert_str(at, &script),
            None => out.push_str(&script),
        }
    }
    out
}

/// A string for a script inside HTML, which cannot end the script element.
fn script_string(text: &str) -> String {
    serde_json::to_string(text)
        .unwrap_or_default()
        .replace("</", "<\\/")
}

/// The redirect page of `latest/` that leads to `target`.
fn stub(target: &str, origin: &str, title: &str) -> String {
    let attr = escape::html(target);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>{title}</title>\n<link rel=\"canonical\" href=\"{origin}{attr}\">\n<meta http-equiv=\"refresh\" content=\"0; url={attr}\">\n<script>location.replace({}+location.hash)</script>\n</head>\n<body>\n<p>The latest version of this page is at <a href=\"{attr}\">{attr}</a>.</p>\n</body>\n</html>\n",
        script_string(target),
        title = escape::html(title),
        origin = escape::html(origin),
    )
}

/// The root page: a refresh to `target`, and the list of the versions as links.
fn root_page(title: &str, target: &Listed, list: &List) -> String {
    let mut items = String::new();
    for v in &list.versions {
        let latest = list.latest.as_deref() == Some(&v.name);
        items.push_str(&format!(
            "<li><a href=\"{}\">{}</a></li>\n",
            escape::html(&v.url),
            escape::html(&label(&v.title, v.status, latest))
        ));
    }
    let url = escape::html(&target.url);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{title}</title>\n<meta http-equiv=\"refresh\" content=\"0; url={url}\">\n<style>body {{ max-width: 40em; margin: 3em auto; padding: 0 16px; font: 16px/1.6 system-ui, sans-serif; }}</style>\n</head>\n<body>\n<h1>{title}</h1>\n<p>The documentation opens at version <a href=\"{url}\">{version}</a>.</p>\n<h2>Versions</h2>\n<ul>\n{items}</ul>\n</body>\n</html>\n",
        title = escape::html(title),
        version = escape::html(&target.title),
    )
}

/// Writes the root files of a set of versions from the declared list, and returns the number of
/// versions.
///
/// It stops before it writes anything when a declared version has no directory, or when a
/// directory holds a version that is not declared.
pub fn refresh(cfg: &Config, root: &Path, root_url: &str) -> Result<usize> {
    if cfg.version.is_empty() {
        bail!("the properties declare no [[version]], so there is no root to refresh");
    }
    if root.join(MARKER).exists() {
        bail!(
            "{} holds a site built without versions, so it has no root to refresh",
            root.display()
        );
    }
    let root_path = path_of(root_url);
    let mut seen = BTreeSet::new();
    let mut versions = Vec::new();
    for v in &cfg.version {
        check_name(&v.name)?;
        if !seen.insert(v.name.as_str()) {
            bail!("the version '{}' is declared twice", v.name);
        }
        let dir = root.join(&v.name);
        if !dir.join(MARKER).is_file() || !dir.join(PAGES).is_file() {
            bail!(
                "the version '{}' is declared, and {} holds no version: build it with --docs-version {} first",
                v.name,
                dir.display(),
                v.name
            );
        }
        versions.push(Listed {
            name: v.name.clone(),
            title: v.title.clone().unwrap_or_else(|| v.name.clone()),
            status: v.status,
            url: format!("{root_path}/{}/", v.name),
        });
    }
    let mut found: Vec<String> = Vec::new();
    for entry in fs::read_dir(root).with_context(|| format!("cannot read {}", root.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir()
            && !name.starts_with('.')
            && name != ALIAS
            && entry.path().join(MARKER).exists()
            && !seen.contains(name.as_str())
        {
            found.push(name);
        }
    }
    found.sort();
    if !found.is_empty() {
        bail!(
            "{} holds versions that the properties do not declare: {}",
            root.display(),
            found.join(", ")
        );
    }
    let latest = versions
        .iter()
        .find(|v| v.status == Status::Release)
        .cloned();
    let list = List {
        format: FORMAT,
        latest: latest.as_ref().map(|v| v.name.clone()),
        versions,
    };
    let alias = match &latest {
        Some(v) => {
            let pages: Pages = read_json(&root.join(&v.name).join(PAGES))?
                .with_context(|| format!("cannot read the pages of the version '{}'", v.name))?;
            Some((v.clone(), pages))
        }
        None => None,
    };

    let json = serde_json::to_string_pretty(&list).context("cannot write the list")? + "\n";
    write_atomic(&root.join(LIST), &json)?;
    let (new, old) = siblings(root, ALIAS);
    let alias_dir = root.join(ALIAS);
    match alias {
        Some((v, pages)) => {
            remove_dir_if_any(&new)?;
            let origin = origin_of(root_url);
            let title = format!("{} {}", cfg.site.title, v.title);
            for rel in pages.pages.keys() {
                let dir = new.join(rel.trim_matches('/'));
                fs::create_dir_all(&dir)?;
                fs::write(
                    dir.join("index.html"),
                    stub(&format!("{}{rel}", v.url), origin, &title),
                )?;
            }
            fs::create_dir_all(&new)?;
            swap(&new, &alias_dir, &old)?;
        }
        None => remove_dir_if_any(&alias_dir)?,
    }
    let target = latest.as_ref().unwrap_or(&list.versions[0]);
    write_atomic(
        &root.join(ROOT_PAGE),
        &root_page(&cfg.site.title, target, &list),
    )?;
    Ok(list.versions.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_one_safe_segment_that_the_root_does_not_use() {
        for good in ["0.3", "3.5-dev", "v3", "2026.10", "main_2"] {
            assert!(check_name(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a\\b",
            "a b",
            "a#b",
            "latest",
            "versions.json",
            "index.html",
        ] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_manifest_reads_back_what_it_wrote_and_an_empty_marker_owns_everything() {
        let files: BTreeSet<String> = ["a/index.html", "search.json"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let text = Manifest::text("0.3", &files);
        assert!(text.starts_with(&format!("dkt {DKT}\nversion 0.3\n")));
        let back = Manifest::parse(&text);
        assert_eq!(back.dkt.as_deref(), Some(DKT));
        assert_eq!(back.files, Some(files));
        assert_eq!(Manifest::parse(""), Manifest::default());
    }

    #[test]
    fn the_origin_is_the_scheme_and_the_host() {
        assert_eq!(origin_of("https://x.io/docs/"), "https://x.io");
        assert_eq!(origin_of("http://h:8"), "http://h:8");
        assert_eq!(origin_of("/docs"), "");
    }

    #[test]
    fn a_label_tells_the_state_in_words() {
        assert_eq!(label("1.1", Status::Release, true), "1.1 (latest)");
        assert_eq!(label("1.0", Status::Release, false), "1.0");
        assert_eq!(label("2.0", Status::Dev, false), "2.0 (development)");
        assert_eq!(label("0.9", Status::Deprecated, false), "0.9 (deprecated)");
    }

    #[test]
    fn a_page_gets_the_meta_the_style_and_the_script_once() {
        let page = "<html><head></head><body>x</body></html>";
        let once = with_versions(page, "/d/1.1", "/d", "1.1");
        assert_eq!(
            once,
            "<html><head><meta name=\"dkt-versions\" content=\"/d/versions.json\" data-root=\"/d\" data-current=\"1.1\">\n<link rel=\"stylesheet\" href=\"/d/1.1/versions.css\">\n</head><body>x<script src=\"/d/1.1/versions.js\" defer></script>\n</body></html>"
        );
        assert_eq!(with_versions(&once, "/d/1.1", "/d", "1.1"), once);
    }

    #[test]
    fn a_redirect_page_keeps_the_anchor_and_cannot_end_its_script() {
        let page = stub("/d/1.1/a</script>/", "https://x.io", "T");
        assert!(page.contains("location.replace(\"/d/1.1/a<\\/script>/\"+location.hash)"));
        assert!(
            page.contains("<link rel=\"canonical\" href=\"https://x.io/d/1.1/a&lt;/script&gt;/\">")
        );
    }
}

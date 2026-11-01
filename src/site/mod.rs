//! A static site built from Markdown documents and a properties file.
//!
//! The documents stay as they are read on GitHub. The properties file says which of them the site
//! serves and where, and a template sets each page. A link between documents becomes a link
//! between pages, and the check of `dkt lint` runs before anything is written.

pub mod config;
pub mod exclude;
mod links;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;
use minijinja::{Value, context};
use serde::Serialize;

use crate::annotation::Annotation;
use crate::cmd::lint::finding::{DEFAULT_RULES, Rule};
use crate::cmd::lint::rules;
use crate::page::{self, Hooks, Page};
use crate::parallel;
use config::Config;
use links::{LinkMap, Mirrored, Served, normalize};

/// File that marks a directory this module wrote, so that a build only clears its own output.
const MARKER: &str = ".dkt-site";
/// Line length for the check, which only matters if its rule is enabled.
const LINE_LENGTH: usize = 100;

/// What a build is asked to do.
pub struct Options {
    pub config: PathBuf,
    pub out: PathBuf,
    /// Replaces the base address of the properties file, to look at a build elsewhere.
    pub base_url: Option<String>,
    /// Runs the check of the documents first and stops on a finding.
    pub lint: bool,
    /// What to build when it is not the whole site: `home`, `static`, `section:NAME` or
    /// `page:SRC`. The rest of the output directory is left as it is.
    pub only: Vec<String>,
}

/// The part of a site that a build is asked for.
struct Scope {
    all: bool,
    home: bool,
    statics: bool,
    sections: HashSet<String>,
    pages: HashSet<String>,
}

impl Scope {
    fn parse(only: &[String]) -> Result<Scope> {
        let mut scope = Scope {
            all: only.is_empty(),
            home: false,
            statics: false,
            sections: HashSet::new(),
            pages: HashSet::new(),
        };
        for what in only {
            match what.split_once(':') {
                None if what == "home" => scope.home = true,
                None if what == "static" => scope.statics = true,
                Some(("section", name)) if !name.is_empty() => {
                    scope.sections.insert(name.to_string());
                }
                Some(("page", src)) if !src.is_empty() => {
                    scope.pages.insert(src.to_string());
                }
                _ => {
                    bail!("cannot build '{what}', expected home, static, section:NAME or page:SRC")
                }
            }
        }
        Ok(scope)
    }

    fn partial(&self) -> bool {
        !self.all
    }

    fn covers(&self, entry: &Entry) -> bool {
        self.all
            || entry
                .section
                .as_ref()
                .is_some_and(|s| self.sections.contains(s))
            || self.pages.contains(&entry.src)
    }

    fn builds_section(&self, name: &str) -> bool {
        self.all || self.sections.contains(name)
    }

    fn builds_home(&self) -> bool {
        self.all || self.home
    }

    fn builds_statics(&self) -> bool {
        self.all || self.statics
    }
}

/// What a build wrote.
pub struct Report {
    pub pages: usize,
}

use crate::format::Format;

/// The variables of the theme that a site can give a colour.
const COLOR_NAMES: [&str; 10] = [
    "bg",
    "surface",
    "surface2",
    "line",
    "fg",
    "soft",
    "muted",
    "accent",
    "accent-bg",
    "link",
];

/// The style that gives a site its colours, or nothing when it has none. A value is a colour, so
/// it holds nothing that could end a declaration.
fn colors_css(colors: &config::Colors) -> Result<String> {
    let declarations = |set: &BTreeMap<String, String>| -> Result<String> {
        let mut out = String::new();
        for (name, value) in set {
            if !COLOR_NAMES.contains(&name.as_str()) {
                bail!(
                    "unknown color '{name}', expected one of {}",
                    COLOR_NAMES.join(", ")
                );
            }
            if value.is_empty()
                || !value
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "#(),.% -".contains(c))
            {
                bail!("'{value}' is not a colour for '{name}'");
            }
            out.push_str(&format!("--{name}: {value}; "));
        }
        // Links have the colour of the accent unless the site gives them another one.
        if let Some(accent) = set.get("accent")
            && !set.contains_key("link")
        {
            out.push_str(&format!("--link: {accent}; "));
        }
        Ok(out)
    };
    let (light, dark) = (declarations(&colors.light)?, declarations(&colors.dark)?);
    let mut css = String::new();
    if !light.is_empty() {
        css.push_str(&format!(":root {{ {light}}}\n"));
    }
    if !dark.is_empty() {
        css.push_str(&format!(
            "@media (prefers-color-scheme: dark) {{ :root:not([data-theme=\"light\"]) {{ {dark}}} }}\n:root[data-theme=\"dark\"] {{ {dark}}}\n"
        ));
    }
    Ok(css)
}

/// The document of the home page, when the properties name one.
fn home_entry(cfg: &Config, base: &str) -> Result<Option<Entry>> {
    let Some(home) = &cfg.home else {
        return Ok(None);
    };
    let Some(src) = &home.src else {
        return Ok(None);
    };
    Ok(Some(Entry {
        src: src.clone(),
        format: Format::of(home.format.as_deref(), src)?,
        title: None,
        url: format!("{base}/"),
        section: None,
        template: home.template.clone(),
        order: 0,
    }))
}

/// The content of a page without its title and the paragraph that follows it, which a template
/// shows where it wants them.
fn after_lede(content: &str) -> String {
    let Some(rest) = content
        .strip_prefix("<h1")
        .and_then(|c| c.split_once("</h1>"))
        .map(|(_, r)| r.trim_start())
    else {
        return content.to_string();
    };
    match rest.strip_prefix("<p>").and_then(|r| r.split_once("</p>")) {
        Some((_, after)) => after.trim_start().to_string(),
        None => rest.to_string(),
    }
}

/// The Markdown of a document: the file itself, or the conversion of a text that is not Markdown.
fn read_document(root: &Path, entry: &Entry) -> Result<String> {
    let path = root.join(&entry.src);
    let text = fs::read_to_string(&path).with_context(|| format!("cannot read {}", entry.src))?;
    let markdown = entry.format.markdown_in(&text, path.parent());
    Ok(match &entry.title {
        Some(title) => with_title(&markdown, title),
        None => markdown,
    })
}

/// The Markdown with the first level one heading replaced by a title, or one added on top.
fn with_title(markdown: &str, title: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut done = false;
    for line in markdown.split_inclusive('\n') {
        if !done && line.starts_with("# ") {
            out.push_str(&format!("# {title}\n"));
            done = true;
        } else {
            out.push_str(line);
        }
    }
    if done {
        out
    } else {
        format!("# {title}\n\n{markdown}")
    }
}

#[derive(Clone)]
struct Entry {
    src: String,
    format: Format,
    /// The title the properties give, which replaces the one of the document.
    title: Option<String>,
    url: String,
    section: Option<String>,
    template: String,
    order: i64,
}

/// The file that tells a page of progress what each page holds that a reader can finish. It is
/// not written for a site that has nothing of the kind.
fn write_widgets(
    out: &Path,
    links: &[PageLink],
    pages: &[Searchable],
    kept: Option<(&HashSet<String>, &[String])>,
) -> Result<()> {
    let mut entries: Vec<String> = Vec::new();
    // A build of a part of the site keeps what the file says of the pages it did not build: the
    // pages it built, and the ones under the address of a section it built, are written again.
    if let Some((built, rebuilt)) = kept
        && let Ok(existing) = fs::read_to_string(out.join("widgets.json"))
    {
        for object in crate::search::objects(&existing) {
            let url = object
                .find("\"u\":\"")
                .and_then(|at| object[at + 5..].split('"').next())
                .unwrap_or_default();
            if !built.contains(url) && !rebuilt.iter().any(|prefix| url.starts_with(prefix)) {
                entries.push(object.to_string());
            }
        }
    }
    for (link, page) in links.iter().zip(pages) {
        if page.widgets.is_empty() {
            continue;
        }
        let mut json = String::from("{\"u\":");
        crate::search::quote(&link.url, &mut json);
        json.push_str(",\"t\":");
        crate::search::quote(&link.title, &mut json);
        json.push_str(",\"w\":[");
        for (k, widget) in page.widgets.iter().enumerate() {
            if k > 0 {
                json.push(',');
            }
            json.push_str("{\"i\":");
            crate::search::quote(&widget.id, &mut json);
            json.push_str(",\"k\":");
            crate::search::quote(&widget.kind, &mut json);
            json.push_str(&format!(",\"n\":{}}}", widget.total));
        }
        json.push_str("]}");
        entries.push(json);
    }
    let to = out.join("widgets.json");
    if entries.is_empty() {
        // A site with no such widget has no file.
        let _ = fs::remove_file(&to);
        return Ok(());
    }
    fs::write(&to, format!("[{}]", entries.join(",")))
        .with_context(|| format!("cannot write {}", to.display()))
}

/// What the search keeps of a page once it is written.
struct Searchable {
    headings: Vec<crate::page::Heading>,
    aliases: Vec<crate::page::Alias>,
    no_search: bool,
    widgets: Vec<crate::page::WidgetInfo>,
}

#[derive(Clone, Serialize)]
struct PageLink {
    title: String,
    nav_title: String,
    description: String,
    url: String,
    weight: i64,
    section: Option<String>,
}

#[derive(Serialize)]
struct NavView {
    name: String,
    url: String,
}

#[derive(Serialize)]
struct SectionView {
    name: String,
    sidebar: bool,
    pager: bool,
    title: String,
    description: String,
    url: String,
    pages: Vec<PageLink>,
}

#[derive(Serialize)]
struct NavMark {
    name: String,
    url: String,
    current: bool,
}

#[derive(Serialize)]
struct SiteView<'a> {
    title: &'a str,
    description: &'a str,
    base_url: &'a str,
    base_path: &'a str,
    colors_css: String,
    search: bool,
    previews: bool,
    nav: Vec<NavView>,
    sections: BTreeMap<String, SectionView>,
}

#[derive(Serialize)]
struct HeadingView {
    level: u8,
    id: String,
    text: String,
}

/// The path part of an address, without a trailing slash: `/lokode` for `https://x.io/lokode/`.
fn base_path(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let path = rest.find('/').map_or("", |i| &rest[i..]);
    path.trim_end_matches('/').to_string()
}

/// The files that match a pattern with one `*` in its file name, as paths from the root.
fn expand(root: &Path, pattern: &str) -> Result<Vec<String>> {
    let (dir, file) = pattern.rsplit_once('/').unwrap_or((".", pattern));
    let (before, after) = file
        .split_once('*')
        .with_context(|| format!("the pattern '{pattern}' has no *"))?;
    let mut found = Vec::new();
    let entries = fs::read_dir(root.join(dir)).with_context(|| format!("cannot read {dir}"))?;
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if name.len() >= before.len() + after.len()
            && name.starts_with(before)
            && name.ends_with(after)
        {
            found.push(normalize(&format!("{dir}/{name}")).unwrap_or(name));
        }
    }
    found.sort();
    Ok(found)
}

fn stem(src: &str) -> String {
    let name = src.rsplit('/').next().unwrap_or(src);
    name.rsplit_once('.')
        .map_or(name, |(s, _)| s)
        .to_lowercase()
}

/// The documents the site serves, with the address each one gets.
fn entries(cfg: &Config, root: &Path, base: &str) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for page in &cfg.page {
        let section = page.section.clone();
        match (&page.src, &page.glob) {
            (Some(src), None) => {
                let url = page.url.clone().unwrap_or_else(|| match &section {
                    Some(s) => format!("/{s}/{}/", stem(src)),
                    None => format!("/{}/", stem(src)),
                });
                out.push(Entry {
                    src: src.clone(),
                    format: Format::of(page.format.as_deref(), src)?,
                    title: page.title.clone(),
                    url: format!("{base}{url}"),
                    section,
                    template: page.template.clone(),
                    // The documents of a section come in the order they are written.
                    order: out.len() as i64,
                });
            }
            (None, Some(glob)) => {
                for (i, src) in expand(root, glob)?.into_iter().enumerate() {
                    let name = stem(&src);
                    let place = page.order.iter().position(|o| *o == name);
                    let url = match &section {
                        Some(s) => format!("{base}/{s}/{name}/"),
                        None => format!("{base}/{name}/"),
                    };
                    if page.title.is_some() {
                        bail!("a title goes with a src, and a glob gives many documents");
                    }
                    out.push(Entry {
                        format: Format::of(page.format.as_deref(), &src)?,
                        title: None,
                        src,
                        url,
                        section: section.clone(),
                        template: page.template.clone(),
                        order: place.unwrap_or(page.order.len() + i) as i64,
                    });
                }
            }
            _ => bail!("a [[page]] needs a src or a glob, and not both"),
        }
    }
    Ok(out)
}

/// The check of a site that no document of `dkt lint` has: a link to a file that the site does not serve.
const UNSERVED: &str = "link-unserved";

fn rule(name: &str) -> Result<Rule> {
    Rule::from_str(name, true).map_err(|_| anyhow!("unknown lint rule '{name}'"))
}

/// Runs the check of `dkt lint` over the documents and returns what it found.
fn check(cfg: &Config, root: &Path, documents: &[(&Entry, &str)]) -> Result<Vec<String>> {
    let mut enabled: HashSet<Rule> = DEFAULT_RULES.into_iter().collect();
    for name in &cfg.lint.enable {
        enabled.insert(rule(name)?);
    }
    for name in cfg.lint.disable.iter().filter(|n| *n != UNSERVED) {
        enabled.remove(&rule(name)?);
    }
    let found = parallel::map(documents, |(entry, text)| {
        let path = root.join(&entry.src);
        rules::check(&path, text, &enabled, LINE_LENGTH)
            .into_iter()
            .map(|finding| finding.render(&entry.src))
            .collect::<Vec<_>>()
    });
    Ok(found.into_iter().flatten().collect())
}

/// The theme a site names, or the default one when it names none: a directory with `templates` or `static` next to the properties file,
/// or else a built-in theme. A directory that has neither, such as the `docs` of a project, is
/// not a theme.
fn resolve_theme(
    root: &Path,
    name: Option<&Path>,
) -> Result<(Option<PathBuf>, Option<&'static crate::theme::Builtin>)> {
    let Some(name) = name else {
        return Ok((None, crate::theme::builtin(crate::theme::DEFAULT)));
    };
    let dir = root.join(name);
    if dir.join("templates").is_dir() || dir.join("static").is_dir() {
        return Ok((Some(dir), None));
    }
    if let Some(builtin) = name.to_str().and_then(crate::theme::builtin) {
        return Ok((None, Some(builtin)));
    }
    bail!(
        "the theme {} is neither a directory nor one of the built-in themes: {}",
        name.display(),
        crate::theme::names().join(", ")
    )
}

/// Writes `text` to `path`, making the directories above it.
fn write_file(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

/// Copies the files of `from` that none of the patterns leaves out.
fn copy_dir_excluding(from: &Path, to: &Path, exclude: &[String]) -> Result<()> {
    fn walk(from: &Path, to: &Path, rel: &str, exclude: &[String]) -> Result<()> {
        fs::create_dir_all(to)?;
        for entry in
            fs::read_dir(from).with_context(|| format!("cannot read {}", from.display()))?
        {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let is_dir = entry.file_type()?.is_dir();
            if exclude::matches_any(exclude, &path, is_dir) {
                continue;
            }
            if is_dir {
                walk(&entry.path(), &to.join(&name), &path, exclude)?;
            } else {
                fs::copy(entry.path(), to.join(&name))?;
            }
        }
        Ok(())
    }
    walk(from, to, "", exclude)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from).with_context(|| format!("cannot read {}", from.display()))? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Empties the output directory, but only one this module made.
fn prepare_out(out: &Path) -> Result<()> {
    if out.exists() {
        let empty = fs::read_dir(out)?.next().is_none();
        if !empty && !out.join(MARKER).exists() {
            bail!(
                "{} is not empty and was not made by dkt site, so it is not cleared",
                out.display()
            );
        }
        fs::remove_dir_all(out)?;
    }
    fs::create_dir_all(out)?;
    fs::write(out.join(MARKER), "")?;
    Ok(())
}

fn write_page(out: &Path, base: &str, url: &str, html: &str) -> Result<()> {
    let rel = url.strip_prefix(base).unwrap_or(url).trim_matches('/');
    let dir = if rel.is_empty() {
        out.to_path_buf()
    } else {
        out.join(rel)
    };
    fs::create_dir_all(&dir)?;
    fs::write(
        dir.join("index.html"),
        crate::theme::with_widgets(html, base),
    )?;
    Ok(())
}

/// What the text and the annotations make of a document, before the links are known.
struct Meta {
    title: String,
    nav_title: String,
    description: String,
    weight: i64,
    hidden: bool,
}

fn meta(entry: &Entry, page: &Page) -> Meta {
    let title = page.title.clone().unwrap_or_else(|| stem(&entry.src));
    Meta {
        nav_title: page.nav_title.clone().unwrap_or_else(|| title.clone()),
        description: page.description.clone().unwrap_or_default(),
        weight: page.weight.unwrap_or(entry.order),
        hidden: page.hidden,
        title,
    }
}

fn headings(page: &Page) -> Vec<HeadingView> {
    page.headings
        .iter()
        .map(|h| HeadingView {
            level: h.level,
            id: h.id.clone(),
            text: h.text.clone(),
        })
        .collect()
}

/// A properties file of an earlier version that still gives `repo` or `branch` is refused, since
/// they would be dropped without a word.
fn refuse_the_repository(text: &str, config: &Path) -> Result<()> {
    let Ok(table) = text.parse::<toml::Table>() else {
        return Ok(());
    };
    let Some(site) = table.get("site").and_then(|s| s.as_table()) else {
        return Ok(());
    };
    for key in ["repo", "branch"] {
        if site.contains_key(key) {
            bail!(
                "{}: `{key}` is no longer an option of [site]: give the address of the repository as `repo` in [data], and write a link to a file that the site does not serve as a full address",
                config.display()
            );
        }
    }
    Ok(())
}

/// Builds the site described by the properties file and returns what it wrote.
pub fn build(opts: &Options) -> Result<Report> {
    let text = fs::read_to_string(&opts.config)
        .with_context(|| format!("cannot read {}", opts.config.display()))?;
    refuse_the_repository(&text, &opts.config)?;
    let cfg: Config = toml::from_str(&text)
        .with_context(|| format!("cannot read the properties in {}", opts.config.display()))?;
    let root = opts
        .config
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let base_url = opts
        .base_url
        .clone()
        .unwrap_or_else(|| cfg.site.base_url.clone());
    let base = base_path(&base_url);

    let mut entries = entries(&cfg, &root, &base)?;
    let scope = Scope::parse(&opts.only)?;
    for name in &scope.sections {
        if !cfg.section.iter().any(|s| &s.name == name) {
            bail!("the site has no section '{name}'");
        }
    }
    for src in &scope.pages {
        if !entries.iter().any(|e| &e.src == src) {
            bail!("the site has no page from '{src}'");
        }
    }
    // Each document is read and converted once, and the three passes below share the result.
    let documents = parallel::map(&entries, |entry| {
        scope
            .covers(entry)
            .then(|| read_document(&root, entry))
            .transpose()
    })
    .into_iter()
    .collect::<Result<Vec<Option<String>>>>()?;
    let home_doc = home_entry(&cfg, &base)?.filter(|_| scope.builds_home());
    let home_md = home_doc
        .as_ref()
        .map(|entry| read_document(&root, entry))
        .transpose()?;
    if opts.lint {
        let mut checked: Vec<(&Entry, &str)> = entries
            .iter()
            .zip(&documents)
            .filter_map(|(entry, md)| Some((entry, md.as_deref()?)))
            .collect();
        if let (Some(entry), Some(md)) = (&home_doc, &home_md) {
            checked.push((entry, md));
        }
        let findings = check(&cfg, &root, &checked)?;
        if !findings.is_empty() {
            bail!(
                "the documents do not pass the check:\n{}",
                findings.join("\n")
            );
        }
    }

    // The first pass reads what each document says of itself, so that a hidden one is not linked.
    // A document that is out of the scope is not read, and is taken as served.
    let jobs: Vec<(&Entry, Option<&String>)> = entries
        .iter()
        .zip(documents.iter().map(Option::as_ref))
        .collect();
    let metas = parallel::map(&jobs, |(entry, md)| {
        md.map(|md| meta(entry, &page::render(md, None)))
    });
    let mut kept = Vec::new();
    let mut kept_meta = Vec::new();
    let mut kept_md = Vec::new();
    let mut served_entries = Vec::new();
    for ((entry, meta), md) in entries.drain(..).zip(metas).zip(documents) {
        if meta.as_ref().is_some_and(|m| m.hidden) {
            continue;
        }
        served_entries.push((entry.src.clone(), entry.url.clone()));
        if let (Some(meta), Some(md)) = (meta, md) {
            kept.push(entry);
            kept_meta.push(meta);
            kept_md.push(md);
        }
    }
    let entries = kept;

    let served: HashMap<String, String> = served_entries
        .iter()
        .filter_map(|(src, url)| Some((normalize(src)?, url.clone())))
        .collect();
    let assets: Vec<Served> = cfg
        .asset
        .iter()
        .map(|a| Served {
            src: a.src.trim_matches('/').to_string(),
            url: format!("{base}{}", a.url),
            exclude: a.exclude.clone(),
        })
        .collect();
    let mirrors: Vec<Mirrored> = cfg
        .mirror
        .iter()
        .map(|m| Mirrored {
            url: m.url.clone(),
            src: m.src.trim_matches('/').to_string(),
            exclude: m.exclude.clone(),
        })
        .collect();
    let links = LinkMap::new(&root, served, assets, mirrors);
    if opts.lint && !cfg.lint.disable.iter().any(|n| n == UNSERVED) {
        let mut found = Vec::new();
        for (entry, md) in entries.iter().zip(&kept_md) {
            let path = root.join(&entry.src);
            for (line, col, target) in rules::local_targets(&path, md) {
                if let Some(rel) = links.unserved(&entry.src, &target) {
                    found.push(format!(
                        "{}:{line}:{col}: {UNSERVED}: {rel} exists, and the site does not serve it",
                        entry.src
                    ));
                }
            }
        }
        for (entry, md) in entries.iter().zip(&kept_md) {
            let path = root.join(&entry.src);
            for (line, col, target) in rules::external_targets(&path, md) {
                if let Some(rel) = links.unserved_mirror(&target) {
                    let what = if links.exists(&rel) {
                        "exists, and the site does not serve it"
                    } else {
                        "is not a file of the sources"
                    };
                    found.push(format!(
                        "{}:{line}:{col}: {UNSERVED}: {rel} {what}",
                        entry.src
                    ));
                }
            }
        }
        if !found.is_empty() {
            bail!(
                "the documents link to files that the site does not serve:\n{}",
                found.join("\n")
            );
        }
    }

    let prefix = |url: String| {
        if url.starts_with('/') {
            format!("{base}{url}")
        } else {
            url
        }
    };
    let figure = |note: &Annotation| -> Option<String> {
        let name = note.get("name")?;
        let src = cfg.figures.src.as_deref()?.replace("{name}", name);
        let dark = cfg
            .figures
            .dark
            .as_deref()
            .map(|d| d.replace("{name}", name));
        note.with_images(prefix(src), dark.map(prefix)).figure()
    };

    let links_of_pages: Vec<PageLink> = entries
        .iter()
        .zip(&kept_meta)
        .map(|(entry, meta)| PageLink {
            title: meta.title.clone(),
            nav_title: meta.nav_title.clone(),
            description: meta.description.clone(),
            url: entry.url.clone(),
            weight: meta.weight,
            section: entry.section.clone(),
        })
        .collect();

    let mut sections: BTreeMap<String, SectionView> = BTreeMap::new();
    for s in &cfg.section {
        let url = prefix(s.url.clone().unwrap_or_else(|| format!("/{}/", s.name)));
        sections.insert(
            s.name.clone(),
            SectionView {
                name: s.name.clone(),
                sidebar: s.sidebar,
                pager: s.pager,
                title: s.title.clone().unwrap_or_else(|| s.name.clone()),
                description: s.description.clone(),
                url,
                pages: Vec::new(),
            },
        );
    }
    for link in &links_of_pages {
        if let Some(view) = link.section.as_ref().and_then(|n| sections.get_mut(n)) {
            view.pages.push(link.clone());
        }
    }
    for view in sections.values_mut() {
        view.pages
            .sort_by(|a, b| (a.weight, &a.title).cmp(&(b.weight, &b.title)));
    }

    let site = SiteView {
        title: &cfg.site.title,
        description: &cfg.site.description,
        base_url: &base_url,
        base_path: &base,
        colors_css: colors_css(&cfg.colors)?,
        search: cfg.site.search,
        previews: cfg.site.previews && cfg.site.search,
        nav: cfg
            .nav
            .iter()
            .map(|n| NavView {
                name: n.name.clone(),
                url: prefix(n.url.clone()),
            })
            .collect(),
        sections,
    };
    let site_value = Value::from_serialize(&site);
    let data_value = Value::from_serialize(&cfg.data);

    let (theme_dir, builtin) = resolve_theme(&root, cfg.site.theme.as_deref())?;
    let mut template_dirs = vec![root.join(&cfg.site.templates)];
    if let Some(dir) = &theme_dir {
        template_dirs.push(dir.join("templates"));
    }
    let env = crate::template::environment(template_dirs, builtin);
    let home_url = format!("{base}/");
    let nav_for = |url: &str, section: Option<&str>| -> Vec<NavMark> {
        cfg.nav
            .iter()
            .map(|n| {
                let target = prefix(n.url.clone());
                let current = url == target
                    || (n.section.is_some() && n.section.as_deref() == section)
                    || (target != home_url && url.starts_with(&target));
                NavMark {
                    name: n.name.clone(),
                    url: target,
                    current,
                }
            })
            .collect()
    };
    let render =
        |template: &str, page: Value, section: Value, nav: Vec<NavMark>| -> Result<String> {
            env.get_template(template)
                .with_context(|| format!("cannot load the template {template}"))?
                .render(context! {
                    site => site_value.clone(),
                    data => data_value.clone(),
                    page => page,
                    section => section,
                    nav => Value::from_serialize(nav),
                })
                .with_context(|| format!("cannot render the template {template}"))
        };

    if scope.partial() {
        // The rest of the site is already there, so nothing is cleared.
        fs::create_dir_all(&opts.out)?;
        let marker = opts.out.join(MARKER);
        if !marker.exists() {
            fs::write(marker, "")?;
        }
    } else {
        prepare_out(&opts.out)?;
    }
    // The second pass renders with the links of the site, and writes each page as soon as it is set.
    let jobs: Vec<(&Entry, &PageLink, &String)> = entries
        .iter()
        .zip(&links_of_pages)
        .zip(&kept_md)
        .map(|((entry, link), md)| (entry, link, md))
        .collect();
    let pages = parallel::map(&jobs, |(entry, link, md)| -> Result<Searchable> {
        let rewrite = |target: &str| links.resolve(&entry.src, target);
        let mut page = page::render_with(
            md,
            &Hooks {
                rewrite: Some(&rewrite),
                figure: Some(&figure),
            },
        );
        let view = context! {
            title => &link.title,
            description => &link.description,
            headings => Value::from_serialize(headings(&page)),
            content => Value::from_safe_string(std::mem::take(&mut page.content)),
            path => &entry.src,
            url => &entry.url,
            section => &entry.section,
            nav_title => &link.nav_title,
            weight => link.weight,
        };
        let nav = nav_for(&entry.url, entry.section.as_deref());
        let html = render(&entry.template, view, Value::UNDEFINED, nav)?;
        write_page(&opts.out, &base, &entry.url, &html)?;
        Ok(Searchable {
            headings: page.headings,
            aliases: page.aliases,
            no_search: page.no_search,
            widgets: page.widgets,
        })
    })
    .into_iter()
    .collect::<Result<Vec<_>>>()?;
    drop(kept_md);
    let mut written = pages.len();
    for s in cfg.section.iter().filter(|s| scope.builds_section(&s.name)) {
        let view = &site.sections[&s.name];
        let page =
            context! { title => &view.title, description => &view.description, url => &view.url };
        let nav = nav_for(&view.url, Some(&s.name));
        let html = render(&s.template, page, Value::from_serialize(view), nav)?;
        write_page(&opts.out, &base, &view.url, &html)?;
        written += 1;
    }
    if let Some(home) = cfg.home.as_ref().filter(|_| scope.builds_home()) {
        // The document of the home page gives its content, and its title when it has one.
        let doc = match (&home_doc, &home_md) {
            (Some(entry), Some(md)) => {
                let rewrite = |target: &str| links.resolve(&entry.src, target);
                Some(page::render_with(
                    md,
                    &Hooks {
                        rewrite: Some(&rewrite),
                        figure: Some(&figure),
                    },
                ))
            }
            _ => None,
        };
        let page = context! {
            title => doc.as_ref().and_then(|d| d.title.clone()).unwrap_or_else(|| cfg.site.title.clone()),
            description => doc.as_ref().and_then(|d| d.description.clone()).unwrap_or_else(|| cfg.site.description.clone()),
            content => Value::from_safe_string(doc.as_ref().map(|d| d.content.clone()).unwrap_or_default()),
            rest => Value::from_safe_string(doc.as_ref().map(|d| after_lede(&d.content)).unwrap_or_default()),
            headings => Value::from_serialize(doc.as_ref().map(headings).unwrap_or_default()),
            url => format!("{base}/"),
        };
        let nav = nav_for(&format!("{base}/"), None);
        let html = render(&home.template, page, Value::UNDEFINED, nav)?;
        write_page(&opts.out, &base, &format!("{base}/"), &html)?;
        written += 1;
    }
    if scope.builds_statics() {
        // Every theme gets the widgets, and its own files and the site's replace them.
        for (path, text) in crate::theme::WIDGET_FILES {
            write_file(&opts.out.join(path), text)?;
        }
        if let Some(builtin) = builtin {
            for (path, text) in builtin.static_files() {
                write_file(&opts.out.join(path), text)?;
            }
        }
        if let Some(dir) = &theme_dir {
            let shared = dir.join("static");
            if shared.is_dir() {
                copy_dir(&shared, &opts.out)?;
            }
        }
        if let Some(dir) = &cfg.site.static_dir {
            copy_dir(&root.join(dir), &opts.out)?;
        }
        for asset in &cfg.asset {
            let to = asset.url.trim_matches('/');
            copy_dir_excluding(&root.join(&asset.src), &opts.out.join(to), &asset.exclude)?;
        }
    }
    if cfg.site.search {
        // A page of a section is told from the same page of another section by the title of the
        // section, as in `Configuration Manual - HAProxy 3.4`.
        let titles: Vec<String> = links_of_pages
            .iter()
            .map(
                |link| match link.section.as_ref().and_then(|n| site.sections.get(n)) {
                    Some(view) => format!("{} - {}", link.title, view.title),
                    None => link.title.clone(),
                },
            )
            .collect();
        let index = crate::search::entries(
            links_of_pages
                .iter()
                .zip(&pages)
                .zip(&titles)
                .filter(|((_, page), _)| !page.no_search)
                .map(|((link, page), title)| crate::search::Source {
                    title,
                    description: &link.description,
                    url: &link.url,
                    headings: &page.headings,
                    aliases: &page.aliases,
                }),
        );
        let to = opts.out.join(crate::search::FILE);
        let json = match fs::read_to_string(&to) {
            // A partial build keeps what the index says of the pages it did not build.
            Ok(existing) if scope.partial() => {
                let built: HashSet<String> = links_of_pages.iter().map(|l| l.url.clone()).collect();
                crate::search::merge(&existing, &built, &index)
            }
            _ => crate::search::to_json(&index),
        };
        fs::write(&to, json).with_context(|| format!("cannot write {}", to.display()))?;
    }
    if scope.partial() {
        let built: HashSet<String> = links_of_pages.iter().map(|l| l.url.clone()).collect();
        let rebuilt: Vec<String> = cfg
            .section
            .iter()
            .filter(|s| scope.builds_section(&s.name))
            .map(|s| site.sections[&s.name].url.clone())
            .collect();
        write_widgets(&opts.out, &links_of_pages, &pages, Some((&built, &rebuilt)))?;
    } else {
        write_widgets(&opts.out, &links_of_pages, &pages, None)?;
    }
    Ok(Report { pages: written })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_path_is_the_path_of_the_address() {
        assert_eq!(base_path("https://x.io/lokode/"), "/lokode");
        assert_eq!(base_path("https://x.io"), "");
        assert_eq!(base_path("http://h:8/a/b"), "/a/b");
        assert_eq!(base_path(""), "");
    }

    #[test]
    fn the_stem_is_the_lower_case_name_without_extension() {
        assert_eq!(stem("docs/guides/Using.md"), "using");
        assert_eq!(stem("README.md"), "readme");
    }
}

//! The properties of a site, read from a TOML file.
//!
//! Everything the Markdown does not say lives here: the title of the site, which documents it
//! serves and where, the navigation, the data the templates read, and the checks to relax.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub site: Site,
    /// The links of the menu.
    #[serde(default)]
    pub nav: Vec<Nav>,
    /// The page at the root of the site, when it has one.
    #[serde(default)]
    pub home: Option<Home>,
    /// Groups of pages that get an index page of their own.
    #[serde(default)]
    pub section: Vec<Section>,
    /// The documents the site serves.
    #[serde(default)]
    pub page: Vec<PageEntry>,
    /// Directories whose files the pages point at, copied to the site.
    #[serde(default)]
    pub asset: Vec<Asset>,
    /// Addresses of documents that this site serves from its own tree.
    #[serde(default)]
    pub mirror: Vec<Mirror>,
    /// Where the images of a `figure` annotation given by name are found.
    #[serde(default)]
    pub figures: Figures,
    #[serde(default)]
    pub lint: LintConfig,
    /// Colours that replace those of the theme, for the light and the dark appearance.
    #[serde(default)]
    pub colors: Colors,
    /// Free data for the templates, read there as `data`.
    #[serde(default)]
    pub data: toml::Table,
}

/// The colours of a site, by the name of the variable of the theme.
#[derive(Debug, Default, Deserialize)]
pub struct Colors {
    #[serde(default)]
    pub light: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub dark: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct Site {
    pub title: String,
    #[serde(default)]
    pub description: String,
    /// Where the site is published. Its path is the prefix of every address.
    #[serde(default)]
    pub base_url: String,
    /// Directory of the templates, next to the properties file.
    #[serde(default = "default_templates")]
    pub templates: PathBuf,
    /// Directory copied as it is to the root of the site.
    #[serde(rename = "static")]
    pub static_dir: Option<PathBuf>,
    /// A directory shared by several sites, holding `templates` and `static`. What the site has
    /// in its own `templates` and `static` wins over it.
    pub theme: Option<PathBuf>,
    /// Whether the build writes the index that a theme searches. On unless the site turns it off.
    #[serde(default = "default_search")]
    pub search: bool,
    /// Whether a link to another page shows a card with the title and the description of the page
    /// when the pointer rests on it. Off unless the site turns it on, and it needs the index.
    #[serde(default)]
    pub previews: bool,
}

fn default_search() -> bool {
    true
}

fn default_templates() -> PathBuf {
    PathBuf::from("templates")
}

#[derive(Debug, Deserialize)]
pub struct Nav {
    pub name: String,
    /// Address inside the site, such as `/guides/`.
    pub url: String,
    /// The section whose pages keep this entry marked as the current one.
    pub section: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Home {
    #[serde(default = "default_home_template")]
    pub template: String,
    /// A document whose content the template shows, so that the text of the home page is written
    /// as any other document.
    pub src: Option<String>,
    /// How that document is written, as for a page.
    pub format: Option<String>,
}

fn default_home_template() -> String {
    "index.html".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Section {
    pub name: String,
    /// Address of the index page, `/NAME/` when omitted.
    pub url: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_section_template")]
    pub template: String,
    /// Whether the pages of the section list their siblings in a side menu.
    #[serde(default)]
    pub sidebar: bool,
    /// Whether a page of the section ends with links to the previous and to the next page.
    #[serde(default)]
    pub pager: bool,
    /// The groups of its menu, in order. A section with none lists its pages in one run.
    #[serde(default)]
    pub group: Vec<Group>,
}

#[derive(Debug, Deserialize)]
pub struct Group {
    pub title: String,
    /// The documents of the group, each by its `src` or by a pattern such as `docs/guides/*.md`
    /// that stands for several, ordered by weight and title.
    pub pages: Vec<String>,
}

fn default_section_template() -> String {
    "section.html".to_string()
}

#[derive(Debug, Deserialize)]
pub struct PageEntry {
    /// One document.
    pub src: Option<String>,
    /// Or the documents of a directory, such as `docs/guides/*.md`.
    pub glob: Option<String>,
    /// Address of a single document, `/STEM/` when omitted.
    pub url: Option<String>,
    /// The section the documents belong to, which also prefixes their address.
    pub section: Option<String>,
    /// File stems in the order they come, before the others.
    #[serde(default)]
    pub order: Vec<String>,
    /// The title of the page, in place of the first level one heading of the document. It goes
    /// with a `src`.
    pub title: Option<String>,
    /// How the documents are written: `markdown` or `haproxy`. It comes from the extension of each
    /// file when omitted.
    pub format: Option<String>,
    #[serde(default = "default_page_template")]
    pub template: String,
}

fn default_page_template() -> String {
    "page.html".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Mirror {
    /// Start of the addresses that the documents write, such as `https://host/org/repo/blob/main/`.
    pub url: String,
    /// Directory in the tree of the documents that holds the files behind those addresses.
    pub src: String,
    /// Patterns of the files that stay at their address, written like the lines of a `.gitignore`
    /// file and relative to `src`.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Asset {
    /// Directory in the tree of the documents.
    pub src: String,
    /// Address inside the site where its files are served.
    pub url: String,
    /// Patterns of the files and directories that stay out of the site, written like the lines of a
    /// `.gitignore` file.
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Figures {
    /// Image of a figure, with `{name}` replaced.
    pub src: Option<String>,
    /// Image of its dark variant, when it has one.
    pub dark: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// Rules to leave out of the check that runs before the build.
    #[serde(default)]
    pub disable: Vec<String>,
    /// Rules to add to it.
    #[serde(default)]
    pub enable: Vec<String>,
}

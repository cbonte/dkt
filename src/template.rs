//! A page set in a template.
//!
//! The template is a Jinja file. A name it extends or includes is looked up next to it.
//! It reads `page.title`, `page.description`, `page.content`, `page.headings` and `page.path`,
//! and the variables of the caller as `vars`. The content is already HTML, so it is not escaped.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use minijinja::{AutoEscape, Environment, Value, context};

use crate::page::Page;
use crate::theme::Builtin;

/// Escapes what HTML needs escaped and leaves the slash alone, so an address stays readable.
fn light_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Reads a template from the first of `dirs` that has it, then from the built-in theme. A name that
/// climbs out of them is refused.
fn load(
    dirs: &[PathBuf],
    builtin: Option<&Builtin>,
    name: &str,
) -> Result<Option<String>, minijinja::Error> {
    if name.starts_with('/') || name.split('/').any(|part| part == ".." || part == ".") {
        return Ok(None);
    }
    for dir in dirs {
        match std::fs::read_to_string(dir.join(name)) {
            Ok(text) => return Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("cannot read the template {name}"),
                )
                .with_source(e));
            }
        }
    }
    Ok(builtin.and_then(|b| b.template(name)).map(str::to_owned))
}

/// A template environment that escapes text for HTML, and reads templates from `dirs`, the first
/// directory that has a name winning, and last from a built-in theme.
pub fn environment(dirs: Vec<PathBuf>, builtin: Option<&'static Builtin>) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_loader(move |name| load(&dirs, builtin, name));
    env.set_formatter(|out, state, value| match value.as_str() {
        Some(text) if matches!(state.auto_escape(), AutoEscape::Html) && !value.is_safe() => {
            out.write_str(&light_escape(text))?;
            Ok(())
        }
        _ => minijinja::escape_formatter(out, state, value),
    });
    env
}

/// Renders `page` with the template at `template`. `path` is where the document was read from.
pub fn render(
    template: &Path,
    page: &Page,
    vars: &BTreeMap<String, String>,
    path: &str,
) -> Result<String> {
    let dir = template
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = template
        .file_name()
        .and_then(|n| n.to_str())
        .context("the template has no usable name")?;
    let env = environment(vec![dir.to_path_buf()], None);
    let tmpl = env
        .get_template(name)
        .with_context(|| format!("cannot load {}", template.display()))?;
    let headings: Vec<Value> = page
        .headings
        .iter()
        .map(|h| context! { level => h.level, id => &h.id, text => &h.text })
        .collect();
    let ctx = context! {
        page => context! {
            title => &page.title,
            description => &page.description,
            content => Value::from_safe_string(page.content.clone()),
            headings => headings,
            path => path,
        },
        vars => vars,
    };
    tmpl.render(ctx)
        .with_context(|| format!("cannot render {}", template.display()))
}

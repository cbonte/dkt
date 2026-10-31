//! The characters of a text that HTML reads as markup.

/// The text with `&`, `<`, `>` and `"` written as entities, fit for a text and for an attribute.
pub fn html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        push(c, &mut out);
    }
    out
}

/// Adds a character to `out`, as an entity when HTML would read it as markup.
pub fn push(c: char, out: &mut String) {
    match c {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        c => out.push(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markup_becomes_entities() {
        assert_eq!(html("a<b>&\"c\""), "a&lt;b&gt;&amp;&quot;c&quot;");
        assert_eq!(html("plain"), "plain");
    }
}

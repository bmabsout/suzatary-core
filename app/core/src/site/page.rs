//! One page of the static site as Typst source, and the vocabulary it is
//! printed in.

use prodrome_core::event::Hash;

use crate::escape::escape_string;

/// A page of the site as Typst this module printed: `PREAMBLE`, then one
/// `shell` call whose every string entered through `lit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page(String);

impl Page {
    /// The source, for the compiler.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `body` inside the shell every page shares; `prefix` leads from the
    /// page back to the site's root.
    pub(super) fn of(title: &str, prefix: &str, footer: &str, body: Vec<Expr>) -> Self {
        let shell = call(
            "shell",
            [lit(title), lit(prefix), lit(footer)]
                .into_iter()
                .chain(body),
        );
        Self(format!("{PREAMBLE}#{}\n", shell.0))
    }

    /// The page that stands where the tripwire refused one: it names the
    /// refusal instead of dropping the page.
    #[must_use]
    pub fn refused(path: &str, reason: &str, footer: &str) -> Self {
        let prefix = "../".repeat(path.matches('/').count());
        Self::of(
            path,
            &prefix,
            footer,
            vec![
                tag("h1", [lit(path)]),
                tag(
                    "p",
                    [lit(&format!(
                        "Refused: typst's HTML for this page carried {reason}, so it was not written. \
                         Its objects are under objects/, and the chain page lists them."
                    ))],
                ),
            ],
        )
    }
}

/// The vocabulary every page is printed in. No stylesheet and no script: a
/// hash breaks every eight characters with a `<wbr>`, which copies as
/// nothing, so it fits a phone, and `color-scheme` lets the browser's own
/// defaults follow a dark theme.
const PREAMBLE: &str = "\
#let el(tag, ..kids) = html.elem(tag, kids.pos().join(default: none))
#let hash(h) = h.clusters().chunks(8).map(c => c.join()).join(html.elem(\"wbr\"))
#let facts(..pairs) = el(\"dl\", ..pairs.pos().map(((k, v)) => el(\"dt\", k) + el(\"dd\", v)))
#let rows(head, ..rows) = el(\"table\", el(\"thead\", el(\"tr\", ..head.map(h => el(\"th\", h)))), el(\"tbody\", ..rows.pos().map(r => el(\"tr\", ..r.map(c => el(\"td\", c))))))
#let shell(title, prefix, footer, ..body) = html.elem(\"html\", attrs: (lang: \"en\"), {
  html.elem(\"head\", {
    html.elem(\"meta\", attrs: (charset: \"utf-8\"))
    html.elem(\"meta\", attrs: (name: \"viewport\", content: \"width=device-width, initial-scale=1\"))
    html.elem(\"meta\", attrs: (name: \"color-scheme\", content: \"light dark\"))
    html.elem(\"title\", title)
  })
  html.elem(\"body\", {
    el(\"nav\", link(prefix + \"index.html\", \"todos\"), \" \", link(prefix + \"chain.html\", \"chain\"), \" \", link(prefix + \"objects/\", \"objects/\"))
    el(\"main\", ..body)
    el(\"footer\", footer)
  })
})
";

/// A Typst expression this module printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Expr(String);

/// A string as a Typst string literal: the one way data enters a page.
pub(super) fn lit(s: &str) -> Expr {
    Expr(format!("\"{}\"", escape_string(s).as_str()))
}

fn call(f: &'static str, args: impl IntoIterator<Item = Expr>) -> Expr {
    let args: Vec<String> = args.into_iter().map(|a| a.0).collect();
    Expr(format!("{f}({})", args.join(", ")))
}

/// `<tag>` around its children.
pub(super) fn tag(name: &'static str, kids: impl IntoIterator<Item = Expr>) -> Expr {
    call(
        "el",
        std::iter::once(Expr(format!("\"{name}\""))).chain(kids),
    )
}

/// Children side by side, in a `<span>`.
pub(super) fn span(kids: impl IntoIterator<Item = Expr>) -> Expr {
    tag("span", kids)
}

pub(super) fn strong(kid: Expr) -> Expr {
    call("strong", [kid])
}

pub(super) fn link(href: &str, label: Expr) -> Expr {
    call("link", [lit(href), label])
}

pub(super) fn hash(h: &Hash) -> Expr {
    call("hash", [lit(h.as_str())])
}

/// Hashes separated by commas.
pub(super) fn hashes(hs: &[Hash]) -> Expr {
    span(
        hs.iter()
            .enumerate()
            .flat_map(|(i, h)| (i > 0).then(|| lit(", ")).into_iter().chain([hash(h)])),
    )
}

/// A Typst array; the trailing comma keeps one item an array.
fn array(items: impl IntoIterator<Item = Expr>) -> Expr {
    let items: Vec<String> = items.into_iter().map(|a| a.0).collect();
    Expr(if items.is_empty() {
        "()".to_owned()
    } else {
        format!("({},)", items.join(", "))
    })
}

/// A definition list.
pub(super) fn facts(pairs: Vec<(&str, Expr)>) -> Expr {
    call("facts", pairs.into_iter().map(|(k, v)| array([lit(k), v])))
}

/// A table with a header row.
pub(super) fn rows(head: &[&str], body: impl IntoIterator<Item = Vec<Expr>>) -> Expr {
    call(
        "rows",
        std::iter::once(array(head.iter().map(|h| lit(h)))).chain(body.into_iter().map(array)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escape::{STRING_SPECIALS, no_unescaped_special};

    /// Whatever a hostile string spells, it is one Typst string literal, so
    /// its `#`, `]` and tags are text.
    #[test]
    fn a_string_enters_a_page_only_as_one_literal() {
        let Expr(printed) = lit("a\"]#eval(\"1\")[<script>\\");
        assert_eq!(printed, r#""a\"]#eval(\"1\")[<script>\\""#);
        assert!(no_unescaped_special(
            &printed[1..printed.len() - 1],
            STRING_SPECIALS
        ));
    }

    #[test]
    fn one_item_is_still_an_array() {
        assert_eq!(array([lit("x")]).0, "(\"x\",)");
        assert_eq!(array([]).0, "()");
    }

    #[test]
    fn a_refused_page_leads_back_to_the_root() {
        let page = Page::refused("todo/x.html", "a script element", "generated now");
        assert!(page.as_str().contains("#shell(\"todo/x.html\", \"../\", "));
    }
}

//! Authored markup compiled in the closed world, in this process: the Prodrome's
//! `Project::restricted()`, the same Rust the tab runs as WebAssembly, so the box and the tab
//! admit a text by one rule.
//!
//! The restricted project's library is typst's own with `html` (any element and attribute) and
//! `plugin` (WebAssembly) removed from its global scope and from `std`, so no spelling, field
//! access or `eval` reaches either; its world holds only the files a caller sets, so `read`,
//! `json`, `image` and `#import` find nothing and no package is fetched; it has no font, which
//! HTML export does not need, and no clock. In-process the compile has no time bound, so the
//! box runs it in a child it can kill; this is what that child runs.

pub mod closed;

pub use closed::Closed;
use prodrome_typst_wasm::Project;
use serde::Deserialize;
use suzatary_core::capability::{Diagnostic, Rejected, Typeset};

/// The key the text is compiled under.
const MAIN: &str = "/main.typ";

/// The closed world, compiled here and now.
#[derive(Debug, Clone, Copy, Default)]
pub struct InProcess;

impl Typeset for InProcess {
    fn closed(&self, source: &str) -> Result<String, Rejected> {
        compile(source)
    }
}

/// `source` in the closed world: typst's whole HTML document, or its errors.
pub fn compile(source: &str) -> Result<String, Rejected> {
    let mut project = Project::restricted();
    if let Some(refused) = project.set(MAIN, source) {
        return Err(Rejected::whole(refused));
    }
    read(source, &project.compile(MAIN, "{}"))
}

/// The project's answer, as its JSON spells it.
#[derive(Deserialize)]
struct Answer {
    html: Option<String>,
    diagnostics: Vec<Said>,
}

#[derive(Deserialize)]
struct Said {
    severity: Severity,
    message: String,
    #[serde(default)]
    hints: Vec<String>,
    file: Option<String>,
    from: Option<usize>,
    to: Option<usize>,
}

/// How much a diagnostic weighs: only an error refuses a text.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum Severity {
    Error,
    Warning,
}

/// The answer read into a value: a warning is not a failure, and an error's UTF-16 span over
/// the text becomes the byte span the core counts in.
fn read(source: &str, printed: &str) -> Result<String, Rejected> {
    let answer: Answer = serde_json::from_str(printed)
        .map_err(|e| Rejected::whole(format!("the compile answered an unexpected shape: {e}")))?;
    let diagnostics: Vec<Diagnostic> = answer
        .diagnostics
        .into_iter()
        .filter(|d| match d.severity {
            Severity::Error => true,
            Severity::Warning => false,
        })
        .map(|d| Diagnostic {
            span: match (d.file.as_deref(), d.from, d.to) {
                (Some(MAIN), Some(from), Some(to)) => Some(byte(source, from)..byte(source, to)),
                _ => None,
            },
            message: d.message,
            hints: d.hints,
        })
        .collect();
    match answer.html {
        Some(html) if diagnostics.is_empty() => Ok(html),
        _ if diagnostics.is_empty() => Err(Rejected::whole("typst produced no document")),
        _ => Err(Rejected { diagnostics }),
    }
}

/// The byte offset of a UTF-16 offset into `text`, clamped to its end.
fn byte(text: &str, utf16: usize) -> usize {
    let mut units = 0;
    for (at, c) in text.char_indices() {
        if units >= utf16 {
            return at;
        }
        units += c.len_utf16();
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(source: &str) -> String {
        compile(source).expect_err(source).said(source)
    }

    #[test]
    fn markup_compiles_to_typsts_own_html() {
        let html =
            compile("Renew *passport* by #link(\"https://e.test\")[Friday]").expect("compiles");
        assert!(html.contains("<strong>passport</strong>"), "{html}");
        assert!(
            html.contains("<a href=\"https://e.test\">Friday</a>"),
            "{html}"
        );
        assert!(compile("$ x^2 $\n\n- a\n- b").is_ok());
    }

    /// The library holds neither binding, however it is spelled.
    #[test]
    fn html_and_plugin_are_unreachable_by_any_spelling() {
        assert_eq!(
            refused("#html.elem(\"b\")[x]"),
            "1:2: unknown variable: html"
        );
        assert!(refused("#std.html.elem(\"b\")[x]").contains("does not contain `html`"));
        assert!(refused("#eval(\"html.elem(\\\"b\\\")[x]\")").contains("unknown variable: html"));
        assert!(refused("#eval(\"std.html\")").contains("does not contain `html`"));
        assert!(refused("#plugin(\"x.wasm\")").contains("unknown variable: plugin"));
        assert!(refused("#std.plugin").contains("does not contain `plugin`"));
        let probed = compile("#if \"html\" in dictionary(std) [yes] else [no]").expect("compiles");
        assert!(probed.contains("<p>no</p>"), "{probed}");
    }

    #[test]
    fn nothing_outside_the_text_exists() {
        for reaching in [
            "#read(\"/etc/passwd\")",
            "#import \"@preview/cetz:0.3.0\"",
            "#image(\"x.png\")",
        ] {
            let said = refused(reaching);
            assert!(
                said.contains("was not handed to this compilation"),
                "{said}"
            );
        }
        assert!(refused("#datetime.today()").contains("unable to get the current date"));
    }

    #[test]
    fn a_text_that_does_not_parse_is_refused_where_it_fails() {
        assert!(refused("a ] b").starts_with("1:3: unexpected closing bracket"));
        assert!(refused("Pay $40").contains("unclosed delimiter"));
        assert!(refused("mail ada@x.test").contains("does not exist in the document"));
        // A span past a character outside the BMP is counted in bytes, not UTF-16 units.
        assert!(refused("🙂 ] b").starts_with("1:3: unexpected closing bracket"));
    }

    /// The vectors the tab's restricted project is held to (`web/test/typstedit.test.ts`): a text
    /// is admitted when it compiles and its page passes the tripwire.
    #[test]
    fn the_shared_vectors_are_admitted_and_refused_as_written() {
        #[derive(Deserialize)]
        struct Vector {
            text: String,
            admitted: bool,
        }
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../core/tests/web/closed-world.json"
        );
        let vectors: Vec<Vector> =
            serde_json::from_str(&std::fs::read_to_string(path).expect("the vectors read"))
                .expect("the vectors parse");
        for v in vectors {
            let admitted = compile(&v.text)
                .is_ok_and(|html| suzatary_core::tripwire::suspicious(&html).is_none());
            assert_eq!(admitted, v.admitted, "{}", v.text);
        }
    }

    /// The forms of an escaped title compile to its prose, so every stored body still renders.
    #[test]
    fn escaped_prose_compiles_to_itself() {
        let escaped = suzatary_core::escape::escape_markup("Pay $40 to ada@x.test [#1] *now*");
        let html = compile(escaped.as_str()).expect("escaped prose compiles");
        assert!(html.contains("Pay $40 to ada@x.test [#1] *now*"), "{html}");
    }

    /// A warning leaves the document standing; an error refuses it; a severity the project
    /// does not print is an answer of another shape, never read as either.
    #[test]
    fn only_an_error_refuses_and_an_unknown_severity_is_not_guessed() {
        let said = |severity: &str| {
            format!(
                r#"{{"html":"<p>x</p>","diagnostics":[{{"severity":"{severity}","message":"m","file":"/main.typ","from":0,"to":1}}]}}"#
            )
        };
        assert_eq!(read("x", &said("warning")).expect("a warning"), "<p>x</p>");
        assert_eq!(
            read("x", &said("error")).expect_err("an error").diagnostics[0].span,
            Some(0..1)
        );
        assert!(
            read("x", &said("fatal"))
                .expect_err("not a severity")
                .said("x")
                .contains("unexpected shape")
        );
    }
}

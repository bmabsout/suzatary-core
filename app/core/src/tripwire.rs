//! The tripwire over typst's HTML: one check, run by the box on every compile and cache read and by
//! the tab (through `suzatary-wasm`) on every page it compiles, so the two refuse the same strings.
//!
//! typst escapes all text it emits, and every string a page holds reached typst as a string, so a
//! hit is the impossible case. It refuses, never edits: a sanitiser that repaired markup would be a
//! second reading of HTML. A URL is read as a browser reads one, its character references decoded
//! and every control and space taken out, since a browser strips tabs and newlines from a scheme
//! (typst writes them raw inside an attribute).

use std::sync::LazyLock;

use regex::{Captures, Regex};

/// What a tripwire asks of the HTML: a pattern anywhere, a URL attribute whose value runs, or a
/// tag with an attribute named as a handler.
enum Check {
    Matches(Regex),
    Runs(Regex),
    Handler,
}

impl Check {
    fn hit(&self, html: &str) -> bool {
        match self {
            Check::Matches(pattern) => pattern.is_match(html),
            Check::Runs(attribute) => attribute
                .captures_iter(html)
                .any(|found| runs(value_of(&found))),
            Check::Handler => names(html).any(|name| {
                name.get(..2)
                    .is_some_and(|on| on.eq_ignore_ascii_case("on"))
                    && name.get(2..).is_some_and(|event| {
                        !event.is_empty() && event.bytes().all(|b| b.is_ascii_alphabetic())
                    })
            }),
        }
    }
}

/// The whitespace a browser splits a tag on, and no other: the regex crate's `\s` is Unicode's,
/// so a no-break space or a vertical tab would end a name or a value here and not in the browser.
const SPACE: &str = r"\t\n\x0C\r ";

/// A tag's start: `<` and a letter, as a browser opens one (typst writes `<` in text as `&lt;`).
static TAG: LazyLock<Regex> = LazyLock::new(|| pattern(&format!("<[a-zA-Z][^{SPACE}/>]*")));

/// One attribute at the head of what follows a tag's name: its name, then its value whole,
/// quoted or not, so nothing inside a value is ever read as a name.
static ATTRIBUTE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(&format!(
        r#"^[{SPACE}/]*([^{SPACE}/>][^{SPACE}/>=]*)(?:[{SPACE}]*=[{SPACE}]*(?:"[^"]*"|'[^']*'|[^{SPACE}>]*))?"#
    ))
});

/// The name of every attribute of every tag, in the order written.
fn names(html: &str) -> impl Iterator<Item = &str> {
    TAG.find_iter(html).flat_map(move |tag| {
        let mut rest = &html[tag.end()..];
        std::iter::from_fn(move || {
            let found = ATTRIBUTE.captures(rest)?;
            rest = &rest[found.get(0)?.end()..];
            found.get(1).map(|name| name.as_str())
        })
    })
}

fn pattern(text: &str) -> Regex {
    Regex::new(text).expect("a fixed, tested pattern")
}

static TRIPWIRES: LazyLock<[(&'static str, Check); 7]> = LazyLock::new(|| {
    [
        (
            "a script element",
            Check::Matches(pattern(r"(?i)<\s*script")),
        ),
        ("an inline event handler", Check::Handler),
        (
            "an executable URL",
            Check::Runs(pattern(
                r#"(?i)(?:^|[\s/"'])(?:href|src|action|formaction|xlink:href)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'>]*))"#,
            )),
        ),
        (
            "a base element",
            Check::Matches(pattern(r"(?i)<\s*base[\s/>]")),
        ),
        (
            "a meta refresh",
            Check::Matches(pattern(r#"(?i)[\s/"']http-equiv\s*="#)),
        ),
        ("a form", Check::Matches(pattern(r"(?i)<\s*form[\s/>]"))),
        (
            "an embedded frame or object",
            Check::Matches(pattern(r"(?i)<\s*(?:iframe|frame|object|embed)[\s/>]")),
        ),
    ]
});

/// The schemes that run rather than navigate.
const EXECUTABLE: [&str; 3] = ["javascript:", "vbscript:", "data:"];

/// An attribute's value, whichever of the three quotings it was written in.
fn value_of<'h>(found: &Captures<'h>) -> &'h str {
    (1..=3)
        .find_map(|group| found.get(group))
        .map_or("", |value| value.as_str())
}

/// A character reference's character: numeric ones, with or without their `;`, and the named ones
/// a scheme can be spelled with.
static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    pattern(r"(?i)&(?:#x([0-9a-f]+);?|#([0-9]+);?|(tab|newline|colon|amp|quot|apos|lt|gt);)")
});

/// The named references [`REFERENCE`] reads.
fn named(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "tab" => "\t",
        "newline" => "\n",
        "colon" => ":",
        "amp" => "&",
        "quot" => "\"",
        "apos" => "'",
        "lt" => "<",
        _ => ">",
    }
}

/// Does this URL run when followed? Decoded and stripped of every control and space first, so no
/// spelling of a scheme a browser would still read as one slips past.
fn runs(value: &str) -> bool {
    let decoded = REFERENCE.replace_all(value, |found: &Captures<'_>| {
        // A number naming no character reads as U+FFFD, as a browser reads it.
        let numeric = |group, radix| {
            found.get(group).map(|digits| {
                u32::from_str_radix(digits.as_str(), radix)
                    .ok()
                    .and_then(char::from_u32)
                    .unwrap_or(char::REPLACEMENT_CHARACTER)
            })
        };
        numeric(1, 16)
            .or_else(|| numeric(2, 10))
            .map_or_else(|| named(&found[3]).to_owned(), String::from)
    });
    let bare: String = decoded
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    EXECUTABLE.iter().any(|scheme| bare.starts_with(scheme))
}

/// What is wrong with this HTML, or `None`.
#[must_use]
pub fn suspicious(html: &str) -> Option<&'static str> {
    TRIPWIRES
        .iter()
        .find(|(_, check)| check.hit(html))
        .map(|(reason, _)| *reason)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn a_script_element_with_odd_spacing_and_case_is_caught() {
        assert_eq!(
            suspicious("<ScRiPt>alert(1)</script>"),
            Some("a script element")
        );
        assert_eq!(
            suspicious("< \t script src=\"x\">"),
            Some("a script element")
        );
    }

    #[test]
    fn an_inline_event_handler_is_caught() {
        assert_eq!(
            suspicious("<img ONCLICK = \"steal()\">"),
            Some("an inline event handler")
        );
        assert_eq!(
            suspicious("<div onmouseover=\"x()\">"),
            Some("an inline event handler")
        );
        assert_eq!(
            suspicious("<svg/onload=\"x()\">"),
            Some("an inline event handler")
        );
        assert_eq!(
            suspicious("<img src=\"x\"onerror=\"y()\">"),
            Some("an inline event handler")
        );
        assert_eq!(
            suspicious("<img src='x'onerror=y>"),
            Some("an inline event handler")
        );
        // Whitespace a browser does not split on is part of the name or the value it sits in.
        for html in [
            "<img src=a\u{a0}x=\" onerror=alert(1)//\">",
            "<a x=\u{a0}\" onclick=alert(1)//\">",
            "<a x=\u{0b}\" onclick=alert(1)//\">",
            "<x\u{a0}y=\" onclick=alert(1)//\">",
        ] {
            assert_eq!(
                suspicious(html),
                Some("an inline event handler"),
                "{html:?}"
            );
        }
    }

    #[test]
    fn an_executable_url_is_caught() {
        for html in [
            "<a href=\"JavaScript:alert(1)\">go</a>",
            "<a href = 'data:text/html,x'>go</a>",
            "<a href=\"vbscript:msgbox(1)\">go</a>",
            "<a href=\"java\tscript:alert(1)\">go</a>",
            "<a href=\"java\nscript:alert(1)\">go</a>",
            "<a href=\" \u{1}javascript:alert(1)\">go</a>",
            "<a href=\"java&#x9;script:alert(1)\">go</a>",
            "<a href=\"java&Tab;script&colon;alert(1)\">go</a>",
            "<a href=\"&#106&#97vascript:alert(1)\">go</a>",
            "<a href=javascript:alert(1)>go</a>",
            "<iframe src=\"javascript:alert(1)\">",
            "<button formaction=\"javascript:x\">",
            "<svg><a xlink:href=\"javascript:x\">",
        ] {
            assert_eq!(suspicious(html), Some("an executable URL"), "{html:?}");
        }
    }

    #[test]
    fn the_elements_typst_never_writes_are_caught() {
        assert_eq!(
            suspicious("<base href=\"https://example.test/\">"),
            Some("a base element")
        );
        assert_eq!(
            suspicious("<meta http-equiv=\"refresh\" content=\"0;url=https://example.test\">"),
            Some("a meta refresh")
        );
        assert_eq!(
            suspicious("<form action=\"https://example.test\"><input></form>"),
            Some("a form")
        );
        for html in ["<iframe>", "<object data=\"x\">", "<embed src=\"x\">"] {
            assert_eq!(
                suspicious(html),
                Some("an embedded frame or object"),
                "{html}"
            );
        }
    }

    #[test]
    fn ordinary_typst_html_is_not_suspicious() {
        assert_eq!(suspicious("<p>a <strong>thing</strong></p>"), None);
        assert_eq!(
            suspicious(r#"<a href="https://example.com">a link</a>"#),
            None
        );
        assert_eq!(
            suspicious(r#"<a href="mailto:a@example.com?subject=x&amp;body=y">a</a>"#),
            None
        );
        assert_eq!(
            suspicious(
                "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" \
                 content=\"width=device-width\"><title>x</title></head><body><p>hi</p></body></html>"
            ),
            None
        );
        assert_eq!(suspicious("<p>a base of data, a form of words</p>"), None);
        // A value or a text that only looks like a handler is neither.
        assert_eq!(
            suspicious(r#"<a href="https://x.test/a/onboarding=1">x</a>"#),
            None
        );
        assert_eq!(suspicious("<p>a/one = b</p>"), None);
        assert_eq!(suspicious("<p title=\"a onclick=x\">y</p>"), None);
    }

    #[test]
    fn escaped_text_naming_a_script_is_not_suspicious() {
        assert_eq!(suspicious("<p>&lt;script>alert(1)&lt;/script></p>"), None);
    }

    /// A character a browser takes out of a scheme, or that marks no scheme at all: every C0
    /// control, the space, and the other whitespace, raw or as a character reference.
    fn stray() -> impl Strategy<Value = String> {
        let raw = prop_oneof![
            (0u32..0x20).prop_map(|c| char::from_u32(c).expect("C0")),
            Just(' '),
            Just('\u{a0}'),
            Just('\u{2028}'),
            Just('\u{3000}'),
        ];
        (raw, 0u8..3).prop_map(|(c, spelling)| match spelling {
            0 => c.to_string(),
            1 => format!("&#x{:x};", u32::from(c)),
            _ => format!("&#{};", u32::from(c)),
        })
    }

    proptest! {
        #[test]
        fn the_tripwire_answers_for_any_text(text in ".*") {
            if let Some(reason) = suspicious(&text) {
                prop_assert!(
                    TRIPWIRES.iter().any(|(named, _)| *named == reason),
                    "the tripwire invented a reason"
                );
            }
        }

        #[test]
        fn a_script_element_is_caught_wherever_it_sits(
            before in "[a-z ]{0,40}",
            after in "[a-z ]{0,40}",
        ) {
            let html = format!("{before}<script>{after}");
            prop_assert_eq!(suspicious(&html), Some("a script element"));
        }

        /// Every control and space, anywhere in an executable scheme, in any case, in any URL
        /// attribute and any quoting, is refused.
        #[test]
        fn a_scheme_salted_with_controls_and_spaces_is_still_refused(
            scheme in prop::sample::select(&EXECUTABLE[..]),
            salt in prop::collection::vec((0usize..16, stray()), 0..4),
            upper in prop::collection::vec(any::<bool>(), 16),
            attribute in prop::sample::select(&["href", "src", "action", "formaction", "xlink:href"][..]),
            quote in prop::sample::select(&["\"", "'"][..]),
        ) {
            let mut spelled: Vec<String> = scheme
                .chars()
                .zip(&upper)
                .map(|(c, &up)| if up { c.to_ascii_uppercase() } else { c }.to_string())
                .collect();
            for (at, stray) in salt {
                let at = at % (spelled.len() + 1);
                spelled.insert(at, stray);
            }
            let html = format!("<a {attribute}={quote}{}x{quote}>go</a>", spelled.concat());
            prop_assert_eq!(suspicious(&html), Some("an executable URL"), "{:?}", html);
        }
    }
}

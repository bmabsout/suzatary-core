//! The person this deployment serves, `instance.toml`'s `[persona]`.
//!
//! The manuals stay files, because their prose is the review surface; they carry
//! `{{persona.*}}` placeholders that [`Persona::render`] fills in at read time, so nothing is
//! regenerated when the instance changes. A closed, small set of placeholders, not a template
//! language.

/// Everything a manual or the weather line needs to know about the person: no path, no
/// secret, no mail address. Who they are and where is the instance file's `[persona]`, which is
/// pushed; what they wrote about themselves is [`Prose`], which is not.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Persona {
    /// How the manuals address them: `{{persona.name}}`.
    pub name: String,
    /// The city `suzatary weather` forecasts.
    pub city: String,
    /// The zone the person lives in, by name (`America/New_York`): what a day is to them and
    /// the forecast city's hour. Theirs, so it changes when they move.
    pub zone: crate::zone::Zone,
    /// Their own words, read from the box-local `persona.toml`: never a key of `[persona]`,
    /// so the instance file a push carries cannot hold them.
    #[serde(skip)]
    pub prose: Prose,
}

/// The person's own prose: their email voice (which names correspondents and quotes sent
/// mail), what news matters to them, and the anchors of their scale. Box-local, in the tier
/// `<data>/local`, and carried by the vault, never by a push.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Prose {
    /// Topics worth surfacing in a briefing, each a whole phrase; `{{persona.news_filter}}`
    /// joins them into one English sentence.
    pub news_filter: Vec<String>,
    /// This person's email voice, one rule per line; `{{persona.email_voice}}` renders bullets.
    pub email_voice: Vec<String>,
    /// The priority scale's anchors, each `"value: meaning"`; `{{persona.anchors_table}}`
    /// renders them as a table.
    pub anchors: Vec<String>,
}

impl Default for Persona {
    /// A name that reads as a placeholder, so an unconfigured manual looks
    /// unconfigured rather than named after whoever wrote the fixture.
    fn default() -> Self {
        Self {
            name: "(unconfigured persona)".to_owned(),
            city: String::new(),
            zone: crate::zone::Zone::UTC,
            prose: Prose::default(),
        }
    }
}

impl Persona {
    /// Fill in the placeholders; a template need not mention every one.
    #[must_use]
    pub fn render(&self, template: &str) -> String {
        template
            .replace("{{persona.name}}", &self.name)
            .replace(
                "{{persona.news_filter}}",
                &Self::english_list(&self.prose.news_filter),
            )
            .replace(
                "{{persona.email_voice}}",
                &Self::bullets(&self.prose.email_voice),
            )
            .replace("{{persona.anchors_table}}", &self.anchors_table())
    }

    /// `["a"]` → `"a"`; `["a", "b"]` → `"a and b"`; `["a", "b", "c"]` → `"a, b and c"`.
    fn english_list(items: &[String]) -> String {
        match items {
            [] => "nothing".to_owned(),
            [only] => only.clone(),
            [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
        }
    }

    fn bullets(items: &[String]) -> String {
        items
            .iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The anchors as a two-column Markdown table. An entry with no `:` prints whole in the left
    /// column, so malformed configuration is visible rather than dropped.
    fn anchors_table(&self) -> String {
        use std::fmt::Write as _;
        let mut rows = String::from("| value | means |\n|---|---|");
        for line in &self.prose.anchors {
            let (value, meaning) = line.split_once(':').unwrap_or((line.as_str(), ""));
            let _ = write!(rows, "\n| {} | {} |", value.trim(), meaning.trim());
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persona() -> Persona {
        Persona {
            name: "Ada".to_owned(),
            city: "London".to_owned(),
            zone: crate::zone::Zone::UTC,
            prose: Prose {
                news_filter: vec![
                    "Norway (friends are there)".to_owned(),
                    "housing policy (her lease)".to_owned(),
                ],
                email_voice: vec![
                    "Never em-dashes.".to_owned(),
                    "One ask per message.".to_owned(),
                ],
                anchors: vec![
                    "10: life-threatening, this hour".to_owned(),
                    "90+: parked".to_owned(),
                ],
            },
        }
    }

    /// The name substitutes everywhere, and text with no placeholder passes through unchanged.
    #[test]
    fn the_name_substitutes_and_plain_text_survives() {
        let p = persona();
        assert_eq!(
            p.render("Hi {{persona.name}}, {{persona.name}} again."),
            "Hi Ada, Ada again."
        );
        assert_eq!(p.render("no placeholders here"), "no placeholders here");
    }

    /// The list joins with "and", and an empty list says so rather than dangling.
    #[test]
    fn the_news_filter_joins_as_a_sentence() {
        assert_eq!(
            persona().render("News: {{persona.news_filter}}."),
            "News: Norway (friends are there) and housing policy (her lease)."
        );
        let none = Persona {
            prose: Prose {
                news_filter: Vec::new(),
                ..persona().prose
            },
            ..persona()
        };
        assert_eq!(
            none.render("News: {{persona.news_filter}}."),
            "News: nothing."
        );
    }

    #[test]
    fn the_email_voice_renders_as_bullets_in_order() {
        assert_eq!(
            persona().render("{{persona.email_voice}}"),
            "- Never em-dashes.\n- One ask per message."
        );
    }

    /// One row per anchor, split on the first colon and trimmed.
    #[test]
    fn the_anchors_table_has_a_header_and_one_row_per_entry() {
        assert_eq!(
            persona().render("{{persona.anchors_table}}"),
            "| value | means |\n|---|---|\n| 10 | life-threatening, this hour |\n| 90+ | parked |"
        );
    }

    /// An anchor with no colon prints whole rather than vanishing.
    #[test]
    fn an_anchor_with_no_colon_prints_whole() {
        let odd = Persona {
            prose: Prose {
                anchors: vec!["not a value-meaning pair".to_owned()],
                ..persona().prose
            },
            ..persona()
        };
        assert_eq!(
            odd.render("{{persona.anchors_table}}"),
            "| value | means |\n|---|---|\n| not a value-meaning pair |  |"
        );
    }
}

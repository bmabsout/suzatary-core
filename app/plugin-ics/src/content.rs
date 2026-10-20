//! RFC 5545 §3.1: a feed's content lines, unfolded and split, and the components they nest into.

/// One property: its name in capitals, its parameters, and its value as written (still escaped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Property {
    /// `DTSTART`, `SUMMARY`: names are case-insensitive, so they are kept in capitals.
    pub name: String,
    /// Each parameter's name in capitals and its values, quotes removed.
    pub params: Vec<(String, Vec<String>)>,
    /// Everything after the first colon outside quotes.
    pub value: String,
}

impl Property {
    /// The first value of parameter `name`.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(named, _)| named == name)
            .and_then(|(_, values)| values.first())
            .map(String::as_str)
    }

    /// The value as TEXT (§3.3.11): backslash escapes read, nothing else touched.
    #[must_use]
    pub fn text(&self) -> String {
        unescape(&self.value)
    }
}

/// One `BEGIN:`…`END:` block: its properties in order and the blocks inside it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Component {
    /// `VCALENDAR`, `VEVENT`, `VTIMEZONE`, in capitals.
    pub name: String,
    /// Its own properties.
    pub properties: Vec<Property>,
    /// The components nested in it.
    pub children: Vec<Component>,
}

impl Component {
    /// Its first property called `name`.
    #[must_use]
    pub fn first(&self, name: &str) -> Option<&Property> {
        self.properties
            .iter()
            .find(|property| property.name == name)
    }

    /// Every property called `name`, in order.
    pub fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Property> + 'a {
        self.properties
            .iter()
            .filter(move |property| property.name == name)
    }

    /// Every child called `name`, in order.
    pub fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Component> + 'a {
        self.children.iter().filter(move |child| child.name == name)
    }
}

/// Why a feed is not iCalendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed(pub String);

/// The feed's calendar: every top-level `VCALENDAR`'s children under one component, since a
/// file may hold several and they say nothing apart.
///
/// # Errors
/// A line with no colon, an `END` that closes nothing open, a block left open, or no
/// `VCALENDAR` at all.
pub fn parse(text: &str) -> Result<Component, Malformed> {
    let mut open: Vec<Component> = Vec::new();
    let mut calendar = Component {
        name: "VCALENDAR".to_owned(),
        ..Component::default()
    };
    let mut found = false;
    for (number, line) in unfold(text).into_iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let property = split(&line)
            .ok_or_else(|| Malformed(format!("line {} is not a content line", number + 1)))?;
        match property.name.as_str() {
            "BEGIN" => open.push(Component {
                name: property.value.to_ascii_uppercase(),
                ..Component::default()
            }),
            "END" => {
                let closed = open
                    .pop()
                    .filter(|component| component.name.eq_ignore_ascii_case(&property.value))
                    .ok_or_else(|| {
                        Malformed(format!("END:{} closes nothing open", property.value))
                    })?;
                match open.last_mut() {
                    Some(parent) => parent.children.push(closed),
                    None if closed.name == "VCALENDAR" => {
                        found = true;
                        calendar.properties.extend(closed.properties);
                        calendar.children.extend(closed.children);
                    }
                    None => {}
                }
            }
            _ => {
                if let Some(component) = open.last_mut() {
                    component.properties.push(property);
                }
            }
        }
    }
    if let Some(component) = open.last() {
        return Err(Malformed(format!(
            "BEGIN:{} is never closed",
            component.name
        )));
    }
    if found {
        Ok(calendar)
    } else {
        Err(Malformed("no VCALENDAR".to_owned()))
    }
}

/// §3.1: a line that begins with a space or a tab continues the one before it, less that one
/// character. Either line ending is read; a byte-order mark is not content.
fn unfold(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let raw = raw.strip_suffix('\r').unwrap_or(raw);
        match (raw.strip_prefix([' ', '\t']), lines.last_mut()) {
            (Some(rest), Some(previous)) => previous.push_str(rest),
            _ => lines.push(raw.to_owned()),
        }
    }
    lines
}

/// `NAME;PARAM=a,"b;c":value`, split where the grammar splits it: a colon or semicolon inside
/// double quotes is part of a parameter's value.
fn split(line: &str) -> Option<Property> {
    let mut quoted = false;
    let mut colon = None;
    for (at, character) in line.char_indices() {
        match character {
            '"' => quoted = !quoted,
            ':' if !quoted => {
                colon = Some(at);
                break;
            }
            _ => {}
        }
    }
    let colon = colon?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = pieces(head, ';').into_iter();
    let name = parts.next()?.trim().to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    let params = parts
        .filter_map(|part| {
            let (key, values) = part.split_once('=')?;
            Some((
                key.trim().to_ascii_uppercase(),
                pieces(values, ',')
                    .into_iter()
                    .map(|value| value.trim_matches('"').to_owned())
                    .collect(),
            ))
        })
        .collect();
    Some(Property {
        name,
        params,
        value: value.to_owned(),
    })
}

/// `text` split at `separator` outside double quotes.
fn pieces(text: &str, separator: char) -> Vec<&str> {
    let mut quoted = false;
    let mut start = 0;
    let mut out = Vec::new();
    for (at, character) in text.char_indices() {
        if character == '"' {
            quoted = !quoted;
        } else if character == separator && !quoted {
            out.push(&text[start..at]);
            start = at + character.len_utf8();
        }
    }
    out.push(&text[start..]);
    out
}

/// §3.3.11: `\\`, `\;`, `\,`, `\n` and `\N`; a backslash before anything else stands for that
/// character, as most writers mean it.
#[must_use]
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character == '\\' {
            match characters.next() {
                Some('n' | 'N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(character);
        }
    }
    out
}

/// Every value of a property that holds a comma-separated list (`EXDATE`, `RDATE`, `CATEGORIES`).
#[must_use]
pub fn list(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folded_line_is_one_line_less_its_first_space() {
        let feed = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nSUMMARY:Plan\r\n ning the\r\n\t week\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
        let calendar = parse(feed).expect("a calendar");
        let event = calendar.children("VEVENT").next().expect("an event");
        assert_eq!(
            event.first("SUMMARY").map(Property::text).as_deref(),
            Some("Planning the week")
        );
    }

    #[test]
    fn a_quoted_parameter_may_hold_a_colon_and_a_semicolon() {
        let property =
            split(r#"ATTENDEE;CN="Lee, Ada: chair; host";ROLE=CHAIR:mailto:ada@example.test"#)
                .expect("a property");
        assert_eq!(property.name, "ATTENDEE");
        assert_eq!(property.param("CN"), Some("Lee, Ada: chair; host"));
        assert_eq!(property.param("ROLE"), Some("CHAIR"));
        assert_eq!(property.value, "mailto:ada@example.test");
    }

    #[test]
    fn text_reads_its_escapes() {
        assert_eq!(
            unescape(r"Room 4\, east wing\; bring a pen\nand paper\\ink"),
            "Room 4, east wing; bring a pen\nand paper\\ink"
        );
    }

    #[test]
    fn names_are_read_in_any_case_and_nesting_is_kept() {
        let feed = "begin:vcalendar\nbegin:vevent\nsummary:x\nbegin:valarm\naction:DISPLAY\nend:valarm\nend:vevent\nend:vcalendar\n";
        let calendar = parse(feed).expect("a calendar");
        let event = calendar.children("VEVENT").next().expect("an event");
        assert!(event.first("SUMMARY").is_some());
        assert_eq!(event.children("VALARM").count(), 1);
        assert!(
            event.first("ACTION").is_none(),
            "an alarm's property is the alarm's"
        );
    }

    #[test]
    fn what_is_not_a_calendar_is_said() {
        assert!(parse("<html>sign in</html>").is_err());
        assert!(parse("BEGIN:VCALENDAR\nBEGIN:VEVENT\nEND:VCALENDAR\n").is_err());
        assert!(parse("BEGIN:VCALENDAR\n").is_err());
    }
}

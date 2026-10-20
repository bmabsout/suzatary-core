//! What a page reads, printed as Typst and compiled by a [`crate::capability::Render`].
//!
//! Every piece of data enters as a string literal through [`escape_string`], so it is text
//! whatever it spells; the one exception is a trusted record's markup, evaluated in place (see
//! [`Prose`]). The HTML carries classes and no colour, so one render serves both themes.

use std::cmp::Ordering;
use std::num::NonZeroU32;

use crate::zone::Zone;
use chrono::{NaiveDateTime, NaiveTime};
use prodrome_core::fpl::{Delta, Explanation, Instant, Note, Scalar, total_seconds};
use prodrome_core::observe::{Observation, Rounding};
use prodrome_core::term::{CurvePoint, TermF};

use crate::escape::{Escaped, escape_string};
use crate::wire;

/// Typst this crate printed for a page. Only the printers below construct one, so a route
/// cannot hand a [`crate::capability::Render`] Typst it assembled from a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page(String);

impl Page {
    /// The source, for the compiler.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn of(body: &str) -> Self {
        Self(format!("{PREAMBLE}{body}"))
    }
}

/// The vocabulary every page is printed in: an element with a class, and one
/// without. Ours, so it may use the `html` module a model's block may not.
const PREAMBLE: &str = "\
#let el(tag, class, ..body) = html.elem(tag, attrs: (class: class), ..body)
#let bare(tag, ..body) = html.elem(tag, ..body)
#let chevron = html.elem(\"svg\", attrs: (viewBox: \"0 0 24 24\", width: \"22\", height: \"22\", fill: \"none\", stroke: \"currentColor\", \"stroke-width\": \"1.7\", \"stroke-linecap\": \"round\", \"stroke-linejoin\": \"round\", \"aria-hidden\": \"true\"), html.elem(\"path\", attrs: (d: \"M8.5 5.5l7 6.5-7 6.5\")))
#let fold(summary, ..body) = html.elem(\"details\", attrs: (class: \"why\", open: \"\"), { html.elem(\"summary\", { summary; chevron }); body.pos().join() })
";

/// A string, as a Typst string literal.
fn lit(s: &str) -> String {
    format!("\"{}\"", escape_string(s).as_str())
}

/// A string, as markup that shows it as text.
fn text(s: &str) -> String {
    format!("#{}", lit(s))
}

/// `<tag class="class">` around already-printed children.
fn el(tag: &str, class: &str, children: &[String]) -> String {
    format!("#el(\"{tag}\", \"{class}\")[{}]", children.concat())
}

fn bare(tag: &str, children: &[String]) -> String {
    format!("#bare(\"{tag}\")[{}]", children.concat())
}

/// What a page shows of a fulfillment: tenths of a percent, to the nearest. A page is printed from
/// this observation of every price on it, so its bytes, and the memo that keys a page by them,
/// move only where the observed value does, whatever the clock says between.
pub const SHOWN: Observation = Observation::new(
    match NonZeroU32::new(1000) {
        Some(levels) => levels,
        None => unreachable!(),
    },
    Rounding::Nearest,
);

/// A fulfillment as a page prints one: [`SHOWN`], "62.3%".
fn pct(value: f64) -> String {
    let step = SHOWN.observe(Some(value)).unwrap_or_default();
    format!("{}.{}%", step / 10, step % 10)
}

/// A node's reading: its number, or `∅` where it has none, never a number standing in.
fn reads(value: Option<f64>) -> String {
    value.map_or_else(|| "∅".to_owned(), pct)
}

// --- Prose: a record's field, rendered or shown as its source ------------------

/// One field of a record, as a page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prose {
    /// A trusted actor's markup that compiles on its own: evaluated in place,
    /// so it reads exactly as its own rendering does.
    Rendered(Escaped),
    /// Anything else, shown as its source text: an untrusted actor's (rendered means
    /// confirmed), or markup Typst refused.
    Source(String),
}

impl Prose {
    fn print(&self, class: &str) -> String {
        match self {
            Prose::Rendered(markup) => format!(
                "#el(\"div\", \"{class} rich\", eval(\"{}\", mode: \"markup\"))",
                escape_string(markup.as_str()).as_str()
            ),
            Prose::Source(source) => el("div", &format!("{class} assource"), &[text(source)]),
        }
    }
}

// --- Why this number -----------------------------------------------------------

/// The core's explanation of one price, read by its closed shape, so a kind the core gains
/// fails a match here rather than printing its fields.
#[derive(Debug, Clone, PartialEq)]
pub struct Explained(Explanation);

impl Explained {
    /// The explanation, to be read.
    #[must_use]
    pub fn of(explanation: Explanation) -> Self {
        Self(explanation)
    }

    /// The first recurrence's last pass, depth first: "tended Sep 13, 12 days
    /// ago", "not tended yet", or `None` for a price with no recurrence.
    #[must_use]
    pub fn pass(&self, zone: Zone) -> Option<String> {
        pass(zone, &self.0)
    }
}

fn pass(z: Zone, node: &Explanation) -> Option<String> {
    match &*node.node {
        TermF::Recur { .. } => Some(tended(z, node)),
        _ => parts(node).into_iter().find_map(|(_, kid)| pass(z, kid)),
    }
}

/// A note the core wrote beside a node, by the key it wrote it under.
fn note<'a>(node: &'a Explanation, key: &str) -> Option<&'a Scalar> {
    match node.notes.get(key)? {
        Note::One(scalar) => Some(scalar),
        Note::Many(_) | Note::Maps(_) => None,
    }
}

fn number(node: &Explanation, key: &str) -> Option<f64> {
    match note(node, key)? {
        Scalar::Float(x) => Some(*x),
        #[allow(clippy::cast_precision_loss)]
        Scalar::Int(n) => Some(*n as f64),
        Scalar::Text(_) | Scalar::Bool(_) => None,
    }
}

/// An instant the core noted, which it prints as `fpl::iso`; "" is none.
fn instant(node: &Explanation, key: &str) -> Option<Instant> {
    match note(node, key)? {
        Scalar::Text(t) => NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S%.f").ok(),
        Scalar::Float(_) | Scalar::Int(_) | Scalar::Bool(_) => None,
    }
}

fn shares(node: &Explanation) -> Vec<f64> {
    match node.notes.get("shares") {
        Some(Note::Many(xs)) => xs
            .iter()
            .filter_map(|x| match x {
                Scalar::Float(x) => Some(*x),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// A parameter in [0, 1] as a reader says it: "60%", "12.5%".
fn share(x: f64) -> String {
    let s = format!("{:.1}", x * 100.0);
    format!("{}%", s.strip_suffix(".0").unwrap_or(&s))
}

/// A plain number, "−4", "1.5".
fn plain(x: f64) -> String {
    let s = format!("{:.2}", x.abs());
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if x < 0.0 {
        format!("−{s}")
    } else {
        s.to_owned()
    }
}

/// An instant as a reader says it: "Sep 30, 02:57", or "Sep 30" at midnight.
/// In UTC, the zone every instant on the chain is in.
fn when(z: Zone, t: Instant) -> String {
    let t = z.wall(t);
    if t.time() == NaiveTime::MIN {
        t.format("%b %-d").to_string()
    } else {
        t.format("%b %-d, %H:%M").to_string()
    }
}

/// A deadline as a reader says it: one reached at the owner's midnight is the end of the day
/// before, the day it is due.
fn due(z: Zone, t: Instant) -> String {
    let wall = z.wall(t);
    match wall.date().pred_opt() {
        Some(day) if wall.time() == NaiveTime::MIN => {
            format!("the end of {}", day.format("%b %-d"))
        }
        _ => when(z, t),
    }
}

fn counted(n: f64, one: &str) -> String {
    let s = plain(n);
    if s == "1" {
        format!("1 {one}")
    } else {
        format!("{s} {one}s")
    }
}

/// A span as a reader says it: "40 minutes", "12 hours", "14 days".
fn span(d: Delta) -> String {
    let hours = total_seconds(d).abs() / 3600.0;
    if hours < 1.0 {
        counted((hours * 60.0).round(), "minute")
    } else if hours < 48.0 {
        counted(hours.round(), "hour")
    } else {
        counted((hours / 24.0 * 10.0).round() / 10.0, "day")
    }
}

fn tended(z: Zone, node: &Explanation) -> String {
    match (instant(node, "tended"), number(node, "agoHours")) {
        (Some(at), Some(ago)) => {
            let days = (ago / 24.0).floor();
            let ago = if days < 1.0 {
                "today".to_owned()
            } else {
                format!("{} ago", counted(days, "day"))
            };
            format!("tended {}, {ago}", z.wall(at).format("%b %-d"))
        }
        _ => "not tended yet".to_owned(),
    }
}

/// Each subterm under the words its slot gives it; "" where the slot is the
/// node's only one and the sentence already says what it is.
fn parts(node: &Explanation) -> Vec<(String, &Explanation)> {
    let one = |kid| vec![(String::new(), kid)];
    match &*node.node {
        TermF::Flat { .. }
        | TermF::Decay { .. }
        | TermF::Curve { .. }
        | TermF::Ref { .. }
        | TermF::Absent => Vec::new(),
        TermF::Conj { terms, .. } => {
            let shares = shares(node);
            terms
                .iter()
                .enumerate()
                .map(|(i, kid)| {
                    let pull = shares
                        .get(i)
                        .map(|s| format!(", {} of the pull", share(*s)))
                        .unwrap_or_default();
                    (format!("Part {}{pull}: ", i + 1), kid)
                })
                .collect()
        }
        TermF::Least { terms } => terms
            .iter()
            .enumerate()
            .map(|(i, kid)| (format!("Version {}: ", i + 1), kid))
            .collect(),
        TermF::Offset { term, .. }
        | TermF::Importance { term, .. }
        | TermF::Shift { term, .. }
        | TermF::Within { term, .. }
        | TermF::Periodic { term, .. } => one(term),
        TermF::Piecewise(schedule) => one(schedule.head()),
        TermF::Gate { gate, body } => vec![
            ("The gate: ".to_owned(), gate),
            ("The body: ".to_owned(), body),
        ],
        TermF::OffsetBy { delta, term } => vec![
            ("Moved by: ".to_owned(), delta),
            ("The part moved: ".to_owned(), term),
        ],
        TermF::After { term, pending, .. } => vec![
            ("Once it is done: ".to_owned(), term),
            ("Until then: ".to_owned(), pending),
        ],
        TermF::Recur { term, pending, .. } => vec![
            ("Since the last pass: ".to_owned(), term),
            ("Before the first pass: ".to_owned(), pending),
        ],
    }
}

/// Which of a todo's specs is in force: "the spec revised on Sep 16 (2
/// revisions in all)", or "the spec as first written".
fn spec(z: Zone, node: &Explanation) -> String {
    instant(node, "since").map_or_else(
        || "the spec as first written".to_owned(),
        |t| {
            format!(
                "the spec revised on {} ({} in all)",
                when(z, t),
                counted(number(node, "pieces").unwrap_or(0.0), "revision")
            )
        },
    )
}

/// A drawn line's points: "80% on Sep 1, then 40% on Oct 1, 09:30 (filing)".
fn through(z: Zone, points: &[CurvePoint]) -> String {
    points
        .iter()
        .map(|pt| {
            let label = if pt.label.is_empty() {
                String::new()
            } else {
                format!(" ({})", pt.label)
            };
            format!("{} on {}{label}", share(pt.value), when(z, pt.at))
        })
        .collect::<Vec<_>>()
        .join(", then ")
}

/// What history says of the event an `After` waits on.
fn bound(node: &Explanation) -> &'static str {
    match note(node, "bound") {
        Some(Scalar::Text(t)) if t == "completed" => {
            "is done, so the follow-up runs from when it was"
        }
        Some(Scalar::Text(t)) if t == "cancelled" => "was cancelled, so this is moot",
        _ => "is not done yet",
    }
}

/// A conflict's sentence: `Least` is how the chain prices writes nobody has settled.
fn least(v: &str, versions: usize) -> String {
    format!(
        "{v}: the most urgent of {versions} versions written concurrently, until the next write \
         settles them."
    )
}

/// One node in one plain sentence, its own fulfillment first.
fn says(z: Zone, node: &Explanation) -> String {
    let v = reads(node.value);
    match &*node.node {
        TermF::Absent => format!("{v}: nothing prices it, so it has no number."),
        TermF::Flat { .. } => format!("{v}, flat: the same whatever the date."),
        TermF::Decay {
            start,
            end,
            end_date,
            lead_up,
            start_date,
        } => {
            let from = start_date
                .map(|t| format!("; 100% before {}", when(z, t)))
                .unwrap_or_default();
            format!(
                "{v} now, falling from {} to {} by {}, over {}{from}.",
                share(*start),
                share(*end),
                due(z, *end_date),
                span(*lead_up)
            )
        }
        TermF::Curve { points } => format!("{v} now, along a drawn line: {}.", through(z, points)),
        TermF::Least { terms } => least(&v, terms.len()),
        TermF::Conj { terms, p } => {
            let floor = number(node, "certifies")
                .map(|c| format!(", so none of them is below {}", share(c)))
                .unwrap_or_default();
            format!(
                "{v}: {} taken together at strictness {}, the lowest pulling hardest{floor}.",
                counted(
                    f64::from(u32::try_from(terms.len()).unwrap_or(u32::MAX)),
                    "part"
                ),
                plain(*p)
            )
        }
        TermF::Offset { delta, .. } => {
            let (way, to) = if *delta < 0.0 {
                ("lowered", "0%")
            } else {
                ("raised", "100%")
            };
            format!(
                "{v}: the part below, {way} {} of the way to {to}.",
                share(delta.abs())
            )
        }
        TermF::Gate { .. } => {
            format!("{v}: the body counts only as far as its gate is met.")
        }
        TermF::Shift { delta, .. } => {
            let at = match delta.cmp(&Delta::zero()) {
                Ordering::Greater => format!("as it will stand in {}", span(*delta)),
                Ordering::Less => format!("as it stood {} earlier", span(*delta)),
                Ordering::Equal => "as it stands now".to_owned(),
            };
            format!("{v}: the part below, {at}.")
        }
        TermF::Within { window, p, .. } => {
            let peak = instant(node, "peakAt")
                .map(|t| format!("; it pulls hardest on {}", when(z, t)))
                .unwrap_or_default();
            format!(
                "{v}: the part below across the next {}, at strictness {}{peak}.",
                span(*window),
                plain(*p)
            )
        }
        TermF::Importance { w, .. } => {
            let how = if *w > 1.0 {
                "sharpened toward urgent"
            } else if *w < 1.0 {
                "softened toward done"
            } else {
                "unchanged"
            };
            format!("{v}: the part below, weighted {}: {how}.", plain(*w))
        }
        TermF::After { event, needs, .. } => {
            let needs = needs
                .map(|n| format!(", and needs {} after it", span(n)))
                .unwrap_or_default();
            format!("{v}: waits on {event}, which {}{needs}.", bound(node))
        }
        TermF::Recur { .. } => format!("{v}: it recurs, {}.", tended(z, node)),
        TermF::Periodic { period, .. } => {
            let round = instant(node, "cycleStart")
                .map(|t| format!("; this round began {}", when(z, t)))
                .unwrap_or_default();
            format!("{v}: it repeats every {}{round}.", span(*period))
        }
        TermF::Piecewise { .. } => format!("{v}, by {}.", spec(z, node)),
        TermF::OffsetBy { .. } => {
            format!("{v}: one part, moved by what the other gives.")
        }
        TermF::Ref { store, entity } => {
            format!("{v}: the price of {}.", referred(store.as_ref(), entity))
        }
    }
}

fn item(z: Zone, label: &str, node: &Explanation) -> String {
    let mut children = vec![el(
        "div",
        "node",
        &[text(&format!("{label}{}", says(z, node)))],
    )];
    let kids = parts(node);
    if !kids.is_empty() {
        children.push(el(
            "ul",
            "tree",
            &kids
                .iter()
                .map(|(label, kid)| item(z, label, kid))
                .collect::<Vec<_>>(),
        ));
    }
    bare("li", &children)
}

/// What a reference names: a todo of this chain, or an entity of another store.
fn referred(store: Option<&String>, entity: &str) -> String {
    store.map_or_else(
        || format!("todo {entity}"),
        |store| format!("{entity} in {store}"),
    )
}

/// "Why this number": the price in one sentence, then its parts in words, or the sentence for
/// a todo with none. The sentence is the piece in force's, and which piece that is gets a line.
fn why_section(z: Zone, tree: Option<&Explained>) -> String {
    let without = |why: &str| {
        el(
            "section",
            "whytree",
            &[
                el("h2", "", &[text("Why this number")]),
                bare("p", &[text(why)]),
            ],
        )
    };
    let Some(Explained(root)) = tree else {
        return without("This todo's price names a todo that cannot be read, so it has no number.");
    };
    if matches!(*root.node, TermF::Absent) {
        return without("The chain records no priority spec for this todo.");
    }
    let mut price = root;
    let mut revision = None;
    while let TermF::Piecewise(schedule) = &*price.node {
        revision.get_or_insert_with(|| spec(z, price));
        price = schedule.head();
    }
    let mut children = vec![
        el(
            "h2",
            "",
            &[text(&format!("Why this number — {}", reads(root.value)))],
        ),
        el("p", "says", &[text(&says(z, price))]),
    ];
    if let Some(revision) = revision {
        children.push(el("p", "pass", &[text(&format!("Priced by {revision}."))]));
    }
    if let Some(pass) = pass(z, root) {
        children.push(el("p", "pass", &[text(&format!("Last pass: {pass}."))]));
    }
    let kids = parts(price);
    if !kids.is_empty() {
        children.push(el(
            "ul",
            "tree",
            &kids
                .iter()
                .map(|(label, kid)| item(z, label, kid))
                .collect::<Vec<_>>(),
        ));
        children.push(el(
            "p",
            "legend",
            &[text(
                "Each part is priced at the moment its parent read it. The box computed every number.",
            )],
        ));
    }
    el("section", "whytree", &children)
}

// --- A todo's reading part -------------------------------------------------------

/// What a todo's content record gives its page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    /// The action line: the page's title.
    pub body: Prose,
    /// The detail, `None` when absent.
    pub detail: Option<Prose>,
}

/// Why the body reads as written, in one line under the header; a body typst drew needs no line.
fn render_note(body: &Prose, unconfirmed: bool) -> Option<&'static str> {
    match (body, unconfirmed) {
        (Prose::Rendered(_), _) => None,
        (Prose::Source(_), true) => Some("Shown the way it was written until you confirm it."),
        (Prose::Source(_), false) => Some("Shown the way it was written: Typst could not draw it."),
    }
}

fn chip(label: &str, kind: &str) -> String {
    el(
        "span",
        &if kind.is_empty() {
            "chip".to_owned()
        } else {
            format!("chip {kind}")
        },
        &[text(label)],
    )
}

/// The header: the number, the body as the title, the id, the chips.
fn head(entry: &wire::Entry, content: Option<&Content>) -> String {
    let authored = entry.authored.as_ref();
    let closed = |word: &str| {
        let day = entry.at.get(..10).unwrap_or_default();
        chip(format!("{word} {day}").trim_end(), "done")
    };
    let state = match entry.state {
        wire::EntryState::Open => chip("open", ""),
        wire::EntryState::Completed => closed("completed"),
        wire::EntryState::Cancelled => closed("cancelled"),
    };
    let subs = authored.map_or(0, |a| a.subtodos.len());
    let done = authored.map_or(0, |a| a.subtodos.iter().filter(|s| s.done).count());
    let category = authored.map_or("", |a| a.category.as_str());
    let mut chips = vec![state];
    if !category.is_empty() {
        chips.push(chip(category, "cat"));
    }
    if let Some(authored) = authored {
        chips.push(chip(authored.block.name(), "kind"));
    }
    if entry.unconfirmed {
        chips.push(chip("unconfirmed", "prov"));
    }
    if subs > 0 {
        chips.push(chip(&format!("{done} of {subs} done"), ""));
    }
    let mut children = vec![el(
        "div",
        "todofulfil",
        &[match entry.value {
            Some(wire::Fulfillment::Number(v)) => el("span", "todopct", &[text(&pct(v))]),
            Some(wire::Fulfillment::Absent(_)) => chip("unpriced", ""),
            None => chip("does not link", ""),
        }],
    )];
    if let Some(content) = content {
        children.push(el("div", "todolead", &[content.body.print("todobody")]));
    }
    children.push(el("code", "todoid", &[text(&entry.todo)]));
    children.push(el("div", "cardmeta", &chips));
    if let Some(note) = content.and_then(|c| render_note(&c.body, entry.unconfirmed)) {
        children.push(el("p", "legend", &[text(note)]));
    }
    el("div", "todohead", &children)
}

/// A todo's reading part, one page: the header first (the page puts its
/// acts under it), then the detail and "Why this number".
#[must_use]
pub fn todo(
    entry: &wire::Entry,
    content: Option<&Content>,
    why: Option<&Explained>,
    z: Zone,
) -> Page {
    let mut parts = vec![head(entry, content)];
    if let Some(detail) = content.and_then(|c| c.detail.as_ref()) {
        parts.push(format!(
            "#fold({})[{}]",
            lit("Detail"),
            detail.print("todobody")
        ));
    }
    parts.push(why_section(z, why));
    Page::of(&parts.concat())
}

// --- The calendar ----------------------------------------------------------------

/// The days of the calendar, one section each, as a page reads them: every title, place and
/// name is a string literal, so a stranger's invite is text whatever it spells.
#[must_use]
pub fn calendar(days: &[crate::calendar::Day]) -> Page {
    let sections: Vec<String> = days
        .iter()
        .map(|day| {
            let heading = bare("h2", &[text(&day.date.format("%A %-d %B").to_string())]);
            let body = if day.entries.is_empty() {
                el("p", "empty", &[text("Nothing on the calendar.")])
            } else {
                el(
                    "ul",
                    "calendarlist",
                    &day.entries.iter().map(calendar_entry).collect::<Vec<_>>(),
                )
            };
            el("section", "calendarday", &[heading, body])
        })
        .collect();
    let legend = el(
        "p",
        "legend",
        &[text(
            "Titles, places and names come from the calendars' feeds: other people's words. A \
             meeting is never a todo.",
        )],
    );
    Page::of(&format!("{}{legend}", sections.concat()))
}

fn calendar_entry(entry: &crate::calendar::Entry) -> String {
    let status = entry.content.status().as_str();
    let mut parts = vec![
        el("span", "calendartime", &[text(&entry.time())]),
        el(
            "span",
            "calendartitle",
            &[text(if entry.content.title().is_empty() {
                "(no title)"
            } else {
                entry.content.title()
            })],
        ),
    ];
    if !entry.content.place().is_empty() {
        parts.push(el("span", "calendarplace", &[text(entry.content.place())]));
    }
    parts.push(el(
        "span",
        "calendarname",
        &[text(entry.id.calendar().label())],
    ));
    if status != "confirmed" {
        parts.push(el("span", "calendarstatus", &[text(status)]));
    }
    if entry.conflicted {
        parts.push(el("span", "calendarstatus", &[text("two syncs disagree")]));
    }
    el("li", &format!("calendarentry {status}"), &parts)
}

/// The same days as plain text, for a page that has no rendering yet.
#[must_use]
pub fn calendar_text(days: &[crate::calendar::Day]) -> String {
    days.iter()
        .map(|day| {
            let head = day.date.format("%A %-d %B").to_string();
            let lines: Vec<String> = day
                .entries
                .iter()
                .map(|entry| {
                    let place = if entry.content.place().is_empty() {
                        String::new()
                    } else {
                        format!(", {}", entry.content.place())
                    };
                    let status = match entry.content.status() {
                        crate::calendar::Status::Confirmed => String::new(),
                        other => format!(" ({})", other.as_str()),
                    };
                    format!(
                        "  {} {}{place}{status}",
                        entry.time(),
                        entry.content.title()
                    )
                })
                .collect();
            if lines.is_empty() {
                format!("{head}\n  nothing")
            } else {
                format!("{head}\n{}", lines.join("\n"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use chrono::Duration;
    use prodrome_core::fpl::{
        Env, Outcome, explained, mk_after, mk_conj, mk_curve, mk_decay, mk_flat, mk_gate,
        mk_importance, mk_offset, mk_offset_by, mk_periodic, mk_piecewise, mk_recur, mk_shift,
        mk_within,
    };
    use prodrome_core::term::{CurvePoint, Term};
    use proptest::prelude::*;

    fn at(s: &str) -> Instant {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").expect("an instant")
    }

    fn flat(v: f64) -> Term {
        mk_flat(v).expect("a flat")
    }

    fn explain(term: &Term, now: &str, env: &Env) -> Explanation {
        let closed = prodrome_core::fpl::Closed::of(term.clone()).expect("closed");
        explained(&closed, at(now), env)
    }

    fn tended_on(todo: &str, when: &str) -> Env {
        let mut env = Env::new();
        env.tended
            .insert(todo.to_owned(), BTreeSet::from([at(when)]));
        env
    }

    #[test]
    fn the_pass_line_reads_the_recurrence_as_the_core_explained_it() {
        let recur = mk_recur(
            "brush".to_owned(),
            at("2026-09-01T08:00:00"),
            flat(0.5),
            flat(0.5),
        )
        .expect("a recur");
        let env = tended_on("brush", "2026-09-13T08:00:00");
        let read = |now| Explained::of(explain(&recur, now, &env)).pass(Zone::UTC);
        assert_eq!(
            read("2026-09-25T13:00:00").as_deref(),
            Some("tended Sep 13, 12 days ago")
        );
        assert_eq!(
            read("2026-09-13T11:00:00").as_deref(),
            Some("tended Sep 13, today")
        );
        assert_eq!(
            read("2026-09-14T14:00:00").as_deref(),
            Some("tended Sep 13, 1 day ago")
        );
        let untended = Explained::of(explain(&recur, "2026-09-25T13:00:00", &Env::new()));
        assert_eq!(untended.pass(Zone::UTC).as_deref(), Some("not tended yet"));
    }

    #[test]
    fn the_pass_line_is_found_under_the_piece_in_force_and_absent_without_one() {
        let recur = mk_recur(
            "brush".to_owned(),
            at("2026-09-01T08:00:00"),
            flat(0.5),
            flat(0.5),
        )
        .expect("a recur");
        let revised =
            mk_piecewise(flat(0.9), vec![(at("2026-09-02T00:00:00"), recur)]).expect("a piecewise");
        let env = tended_on("brush", "2026-09-13T08:00:00");
        assert_eq!(
            Explained::of(explain(&revised, "2026-09-25T13:00:00", &env))
                .pass(Zone::UTC)
                .as_deref(),
            Some("tended Sep 13, 12 days ago")
        );
        assert_eq!(
            Explained::of(explain(&flat(0.5), "2026-09-25T13:00:00", &env)).pass(Zone::UTC),
            None
        );
    }

    fn decay() -> Term {
        mk_decay(
            0.6,
            0.05,
            at("2026-09-30T02:57:04"),
            Duration::hours(336),
            Some(at("2026-09-16T02:57:04")),
        )
        .expect("a decay")
    }

    fn curve() -> Term {
        mk_curve(vec![
            CurvePoint {
                at: at("2026-09-01T00:00:00"),
                value: 0.8,
                label: String::new(),
            },
            CurvePoint {
                at: at("2026-10-01T09:30:00"),
                value: 0.4,
                label: "filing".to_owned(),
            },
        ])
        .expect("a curve")
    }

    /// One term of each closed kind but `Ref` (a linked term holds none), with
    /// the history it is read against and the sentence it must say.
    fn kinds() -> Vec<(Term, Env, String)> {
        let (decay, curve) = (decay(), curve());
        let mut done = Env::new();
        done.outcomes.insert(
            "permit".to_owned(),
            [Some(Outcome::Completed(at("2026-09-20T00:00:00")))].into(),
        );
        vec![
            (flat(0.5), Env::new(), "50.0%, flat: the same whatever the date.".into()),
            (
                decay.clone(),
                Env::new(),
                "{} now, falling from 60% to 5% by Sep 30, 02:57, over 14 days; 100% before Sep 16, 02:57.".into(),
            ),
            (
                curve,
                Env::new(),
                "{} now, along a drawn line: 80% on Sep 1, then 40% on Oct 1, 09:30 (filing).".into(),
            ),
            (
                mk_conj(vec![flat(0.5), flat(0.9)], -4.0).expect("a conj"),
                Env::new(),
                "{}: 2 parts taken together at strictness −4, the lowest pulling hardest, so none of them is below {floor}.".into(),
            ),
            (
                mk_offset(-0.3, flat(0.5)).expect("an offset"),
                Env::new(),
                "{}: the part below, lowered 30% of the way to 0%.".into(),
            ),
            (
                mk_gate(flat(0.5), flat(0.2)).expect("a gate"),
                Env::new(),
                "{}: the body counts only as far as its gate is met.".into(),
            ),
            (
                mk_shift(Duration::hours(72), decay.clone()).expect("a shift"),
                Env::new(),
                "{}: the part below, as it will stand in 3 days.".into(),
            ),
            (
                mk_within(Duration::hours(12), -4.0, decay.clone()).expect("a within"),
                Env::new(),
                "{}: the part below across the next 12 hours, at strictness −4; it pulls hardest on {peak}.".into(),
            ),
            (
                mk_importance(2.0, flat(0.5)).expect("an importance"),
                Env::new(),
                "{}: the part below, weighted 2: sharpened toward urgent.".into(),
            ),
            (
                mk_after(
                    "permit".to_owned(),
                    at("2026-09-10T00:00:00"),
                    flat(0.4),
                    flat(0.9),
                    Some(Duration::hours(3)),
                )
                .expect("an after"),
                done,
                "{}: waits on permit, which is done, so the follow-up runs from when it was, and needs 3 hours after it.".into(),
            ),
            (
                mk_recur(
                    "brush".to_owned(),
                    at("2026-09-01T08:00:00"),
                    flat(0.5),
                    flat(0.5),
                )
                .expect("a recur"),
                tended_on("brush", "2026-09-26T08:00:00"),
                "{}: it recurs, tended Sep 26, 1 day ago.".into(),
            ),
            (
                mk_periodic(Duration::hours(168), at("2026-09-07T00:00:00"), flat(0.5))
                    .expect("a periodic"),
                Env::new(),
                "{}: it repeats every 7 days; this round began Sep 28.".into(),
            ),
            (
                mk_piecewise(flat(0.9), vec![(at("2026-09-16T02:57:04"), flat(0.5))])
                    .expect("a piecewise"),
                Env::new(),
                "{}, by the spec revised on Sep 16, 02:57 (1 revision in all).".into(),
            ),
            (
                mk_offset_by(flat(0.2), flat(0.5)).expect("an offsetBy"),
                Env::new(),
                "{}: one part, moved by what the other gives.".into(),
            ),
        ]
    }

    /// A law per kind: each says itself in one sentence that leads with its own number.
    #[test]
    fn every_kind_says_one_sentence_in_words() {
        let now = "2026-09-28T00:00:00";
        for (term, env, expected) in kinds() {
            let node = explain(&term, now, &env);
            let floor = number(&node, "certifies").map(share).unwrap_or_default();
            let peak = instant(&node, "peakAt")
                .map(|t| when(Zone::UTC, t))
                .unwrap_or_default();
            let expected = expected
                .replacen("{}", &reads(node.value), 1)
                .replace("{floor}", &floor)
                .replace("{peak}", &peak);
            assert_eq!(says(Zone::UTC, &node), expected, "{}", node.node.kind());
        }
        let reference = Explanation {
            value: Some(0.25),
            notes: BTreeMap::new(),
            node: Box::new(TermF::Ref {
                store: None,
                entity: "file-permit".to_owned(),
            }),
        };
        assert_eq!(
            says(Zone::UTC, &reference),
            "25.0%: the price of todo file-permit."
        );
    }

    #[test]
    fn a_conjunctions_members_are_parts_with_their_pull() {
        let conj = mk_conj(vec![flat(0.5), flat(0.9)], -4.0).expect("a conj");
        let node = explain(&conj, "2026-09-28T00:00:00", &Env::new());
        let labels: Vec<String> = parts(&node).into_iter().map(|(l, _)| l).collect();
        let pulls = shares(&node);
        assert_eq!(
            labels,
            [
                format!("Part 1, {} of the pull: ", share(pulls[0])),
                format!("Part 2, {} of the pull: ", share(pulls[1])),
            ]
        );
    }

    /// What the section shows a reader: its text, the `#"…"` literals.
    fn shown(section: &str) -> String {
        let literal = regex::Regex::new(r#"#"((?:[^"\\]|\\.)*)""#).expect("a regex");
        literal
            .captures_iter(section)
            .map(|c| c[1].to_owned())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every field and note name `fpl::explained` writes, and raw spellings a reader must
    /// never see.
    const RAW: [&str; 24] = [
        "start_date",
        "end_date",
        "lead_up",
        "startDate",
        "endDate",
        "leadUpHours",
        "windowHours",
        "deltaHours",
        "periodHours",
        "needsHours",
        "peakAt",
        "peakShare",
        "certifies",
        "shares",
        "agoHours",
        "cycleStart",
        "pieces",
        "anchor",
        "bound",
        "offsetBy",
        "kind",
        "=",
        "terms[",
        "term:",
    ];

    fn instant_strategy() -> impl Strategy<Value = Instant> {
        (0i64..(90 * 24 * 60)).prop_map(|m| at("2026-08-01T00:00:00") + Duration::minutes(m))
    }

    fn unit() -> impl Strategy<Value = f64> {
        0.0f64..=0.98
    }

    fn hours() -> impl Strategy<Value = Delta> {
        (1i64..(40 * 24)).prop_map(Duration::hours)
    }

    fn event() -> impl Strategy<Value = String> {
        prop_oneof![Just("t1".to_owned()), Just("t2".to_owned())]
    }

    fn term_strategy() -> impl Strategy<Value = Term> {
        let leaf = prop_oneof![
            unit().prop_map(flat),
            (
                unit(),
                unit(),
                instant_strategy(),
                hours(),
                proptest::option::of(instant_strategy())
            )
                .prop_map(
                    |(s, e, end, lead, from)| mk_decay(s, e, end, lead, from).expect("a decay")
                ),
            prop::collection::btree_map(instant_strategy(), (unit(), 0u16..999), 1..4).prop_map(
                |points| mk_curve(
                    points
                        .into_iter()
                        .map(|(at, (value, label))| CurvePoint {
                            at,
                            value,
                            label: if label % 2 == 0 {
                                String::new()
                            } else {
                                format!("L{label}")
                            },
                        })
                        .collect()
                )
                .expect("a curve")
            ),
        ];
        leaf.prop_recursive(3, 16, 3, |inner| {
            prop_oneof![
                (prop::collection::vec(inner.clone(), 1..4), -32.0f64..=0.0)
                    .prop_map(|(ts, p)| mk_conj(ts, p).expect("a conj")),
                (-1.0f64..=1.0, inner.clone())
                    .prop_map(|(d, t)| mk_offset(d, t).expect("an offset")),
                (inner.clone(), inner.clone()).prop_map(|(g, b)| mk_gate(g, b).expect("a gate")),
                ((-40i64 * 24)..(40 * 24), inner.clone()).prop_map(|(h, t)| mk_shift(
                    Duration::hours(h),
                    t
                )
                .expect("a shift")),
                (hours(), -32.0f64..=32.0, inner.clone())
                    .prop_map(|(w, p, t)| mk_within(w, p, t).expect("a within")),
                (0.1f64..4.0, inner.clone())
                    .prop_map(|(w, t)| mk_importance(w, t).expect("an importance")),
                (
                    event(),
                    instant_strategy(),
                    inner.clone(),
                    inner.clone(),
                    proptest::option::of(hours())
                )
                    .prop_map(|(e, a, t, p, n)| mk_after(e, a, t, p, n).expect("an after")),
                (event(), instant_strategy(), inner.clone(), inner.clone())
                    .prop_map(|(e, a, t, p)| mk_recur(e, a, t, p).expect("a recur")),
                (hours(), instant_strategy(), inner.clone())
                    .prop_map(|(h, a, t)| mk_periodic(h, a, t).expect("a periodic")),
                (
                    inner.clone(),
                    prop::collection::btree_map(instant_strategy(), inner.clone(), 0..3)
                )
                    .prop_map(
                        |(h, ps)| mk_piecewise(h, ps.into_iter().collect()).expect("a piecewise")
                    ),
                (inner.clone(), inner).prop_map(|(d, t)| mk_offset_by(d, t).expect("an offsetBy")),
            ]
        })
    }

    fn env_strategy() -> impl Strategy<Value = Env> {
        (
            proptest::option::of((any::<bool>(), instant_strategy())),
            prop::collection::btree_set(instant_strategy(), 0..3),
        )
            .prop_map(|(outcome, passes)| {
                let mut env = Env::new();
                if let Some((completed, t)) = outcome {
                    let o = if completed {
                        Outcome::Completed(t)
                    } else {
                        Outcome::Cancelled(t)
                    };
                    env.outcomes.insert("t1".to_owned(), [Some(o)].into());
                }
                env.tended.insert("t2".to_owned(), passes);
                env
            })
    }

    fn walk<'a>(node: &'a Explanation, out: &mut Vec<&'a Explanation>) {
        out.push(node);
        for (_, kid) in parts(node) {
            walk(kid, out);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Whatever the term, "Why this number" is sentences in words (no field name, no ISO
        /// instant), each leading with its own number.
        #[test]
        fn why_this_number_is_words_for_every_term(
            term in term_strategy(),
            now in instant_strategy(),
            env in env_strategy(),
        ) {
            let closed = prodrome_core::fpl::Closed::of(term).expect("closed");
            let root = explained(&closed, now, &env);
            let mut nodes = Vec::new();
            walk(&root, &mut nodes);
            for node in nodes {
                let sentence = says(Zone::UTC, node);
                prop_assert!(sentence.starts_with(&reads(node.value)), "{sentence}");
                prop_assert!(sentence.ends_with('.'), "{sentence}");
            }
            let section = shown(&why_section(Zone::UTC, Some(&Explained::of(root))));
            let iso = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T").expect("a regex");
            prop_assert!(!iso.is_match(&section), "{section}");
            for raw in RAW {
                prop_assert!(!section.contains(raw), "{raw} in {section}");
            }
        }
    }

    #[test]
    fn every_datum_is_one_string_literal() {
        let hostile = Explanation {
            value: Some(0.5),
            notes: BTreeMap::new(),
            node: Box::new(TermF::Ref {
                store: None,
                entity: "a\"]#x[".to_owned(),
            }),
        };
        let section = why_section(Zone::UTC, Some(&Explained::of(hostile)));
        assert!(
            section.contains(r#"[#"50.0%: the price of todo a\"]#x[."]"#),
            "{section}"
        );
    }

    #[test]
    fn a_revised_price_leads_with_the_piece_in_force_and_names_it() {
        let revised = mk_piecewise(flat(0.9), vec![(at("2026-09-16T02:57:04"), flat(0.5))])
            .expect("a piecewise");
        let root = explain(&revised, "2026-09-28T00:00:00", &Env::new());
        let section = shown(&why_section(Zone::UTC, Some(&Explained::of(root))));
        assert_eq!(
            section,
            [
                "Why this number — 50.0%",
                "50.0%, flat: the same whatever the date.",
                "Priced by the spec revised on Sep 16, 02:57 (1 revision in all).",
            ]
            .join("\n"),
        );
    }

    #[test]
    fn a_todo_without_a_spec_says_so() {
        let absent = explain(
            &prodrome_core::fpl::mk_absent(),
            "2026-09-28T00:00:00",
            &Env::new(),
        );
        assert_eq!(absent.value, None, "∅, never a number");
        let section = why_section(Zone::UTC, Some(&Explained::of(absent)));
        assert!(section.contains("The chain records no priority spec for this todo."));
        assert!(!section.contains('%'), "{section}");
        assert!(why_section(Zone::UTC, None).contains("cannot be read, so it has no number"));
    }

    /// The header tells a number, `∅` and an unlinked price apart; only a number draws a
    /// percentage.
    #[test]
    fn the_header_reads_a_number_absent_and_unlinked_apart() {
        let head_of = |value| {
            let page = todo(
                &wire::Entry {
                    value,
                    ..entry(wire::EntryState::Open, false)
                },
                None,
                None,
                Zone::UTC,
            );
            page.as_str().to_owned()
        };
        assert!(head_of(Some(wire::Fulfillment::Number(0.5))).contains(r#"#"50.0%""#));
        let absent = head_of(Some(wire::Fulfillment::of_value(None)));
        assert!(
            absent.contains(r#"#"unpriced""#) && !absent.contains("todopct"),
            "{absent}"
        );
        let unlinked = head_of(None);
        assert!(unlinked.contains(r#"#"does not link""#), "{unlinked}");
    }

    proptest! {
        /// Between an instant and its next change as [`SHOWN`] sees the price, a todo's page
        /// prints the same bytes whatever the clock says, so the memo keyed by them hits; at an
        /// exact change it prints otherwise.
        #[test]
        fn a_page_is_the_same_bytes_until_its_observed_value_moves(
            start in 0.0f64..0.98,
            end in 0.0f64..0.9,
            days in 1i64..30,
            from in 0i64..40 * 24 * 60,
            within in 0.0f64..1.0,
        ) {
            let term = mk_decay(start, end, at("2026-09-30T00:00:00"), Duration::days(days), None)
                .expect("a decay");
            let closed = prodrome_core::fpl::Closed::of(term).expect("closed");
            let env = Env::new();
            let page = |t: Instant| {
                let why = Explained::of(explained(&closed, t, &env));
                let value = prodrome_core::fpl::fulfillment(&closed, t, &env);
                todo(
                    &wire::Entry {
                        value: Some(wire::Fulfillment::of_value(value)),
                        ..entry(wire::EntryState::Open, false)
                    },
                    None,
                    Some(&why),
                    Zone::UTC,
                )
            };
            let t0 = at("2026-09-01T00:00:00") + Duration::minutes(from);
            let next = prodrome_core::observe::next_change(&closed, t0, &env, SHOWN);
            let Some(changes) = next.at else {
                prop_assert_eq!(page(t0 + Duration::days(400)), page(t0));
                return Ok(());
            };
            let span = (changes - t0).num_microseconds().expect("a span");
            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let t = t0 + Duration::microseconds((within * (span - 1) as f64) as i64);
            prop_assert_eq!(page(t), page(t0));
            prop_assert!(next.exact, "a decay is exact");
            prop_assert_ne!(page(changes), page(t0));
        }
    }

    fn entry(state: wire::EntryState, unconfirmed: bool) -> wire::Entry {
        wire::Entry {
            todo: "file-permit".to_owned(),
            state,
            states: vec![state],
            at: "2026-09-20T10:00:00".to_owned(),
            claimed: wire::ClaimedState::Agrees,
            value: Some(wire::Fulfillment::Number(0.623)),
            recurring: false,
            unconfirmed,
            conflicts: std::collections::BTreeMap::new(),
            authored: None,
            candidates: Vec::new(),
            stream: Vec::new(),
        }
    }

    #[test]
    fn prose_is_evaluated_only_when_rendered() {
        let rendered = Prose::Rendered(Escaped::from_chain("*a* \"b\"")).print("todobody");
        assert_eq!(
            rendered,
            r#"#el("div", "todobody rich", eval("*a* \"b\"", mode: "markup"))"#
        );
        let source = Prose::Source("#eval(\"x\")".to_owned()).print("todobody");
        assert_eq!(
            source,
            r##"#el("div", "todobody assource")[#"#eval(\"x\")"]"##
        );
    }

    #[test]
    fn the_reading_leads_with_its_header_and_says_how_the_body_reads() {
        let content = Content {
            body: Prose::Source("File the deed".to_owned()),
            detail: Some(Prose::Source("Mail it certified.".to_owned())),
        };
        let page = todo(
            &entry(wire::EntryState::Open, true),
            Some(&content),
            None,
            Zone::UTC,
        );
        let body = page.as_str().strip_prefix(PREAMBLE).expect("the preamble");
        assert!(body.starts_with(r#"#el("div", "todohead")"#), "{body}");
        assert!(body.contains(r#"#"62.3%""#), "{body}");
        assert!(body.contains(r#"#"unconfirmed""#), "{body}");
        assert!(body.contains("until you confirm it"), "{body}");
        assert!(body.contains(r#"#fold("Detail")"#), "{body}");
        assert!(body.ends_with(&why_section(Zone::UTC, None)), "{body}");
    }

    #[test]
    fn a_resolved_todo_names_its_state_and_day_and_one_without_content_has_no_note() {
        let page = todo(
            &entry(wire::EntryState::Completed, false),
            None,
            None,
            Zone::UTC,
        );
        assert!(page.as_str().contains(r#"#"completed 2026-09-20""#));
        assert!(!page.as_str().contains("Rendered by typst"));
        assert!(!page.as_str().contains("#fold("));
    }

    #[test]
    fn a_body_typst_drew_carries_no_note_under_the_header() {
        let content = Content {
            body: Prose::Rendered(Escaped::from_chain("File the deed")),
            detail: None,
        };
        let page = todo(
            &entry(wire::EntryState::Open, false),
            Some(&content),
            None,
            Zone::UTC,
        );
        let body = page.as_str().strip_prefix(PREAMBLE).expect("the preamble");
        assert!(!body.contains(r#"el("p", "legend""#), "{body}");
    }

    #[test]
    fn a_calendar_page_prints_every_strangers_word_as_a_literal() {
        use crate::calendar::{CalendarName, Content, Day, Entry, OccurrenceId, Status, When};
        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 5).expect("a day");
        let at = |hour| date.and_hms_opt(hour, 0, 0).expect("a time");
        let days = [Day {
            date,
            entries: vec![Entry {
                id: OccurrenceId::new(
                    CalendarName::parse("PLUGIN_ICS_WORK").expect("a name"),
                    "x",
                    "20261005T090000Z",
                )
                .expect("an id"),
                content: Content::new(
                    "Review\"] #eval(\"x\") [".to_owned(),
                    When::timed(at(9), at(10)).expect("a span"),
                    "Room #4".to_owned(),
                    Vec::new(),
                    Status::Cancelled,
                )
                .expect("content"),
                local: Some((at(9), at(10))),
                conflicted: false,
            }],
        }];
        let source = calendar(&days).as_str().to_owned();
        assert!(
            source.contains(r#"#"Review\"] #eval(\"x\") [""#),
            "{source}"
        );
        assert!(
            source.contains(r#"#el("li", "calendarentry cancelled")"#),
            "{source}"
        );
        assert!(source.contains(r#"#"Monday 5 October""#), "{source}");
        let plain = calendar_text(&days);
        assert_eq!(
            plain,
            "Monday 5 October\n  09:00–10:00 Review\"] #eval(\"x\") [, Room #4 (cancelled)"
        );
    }
}

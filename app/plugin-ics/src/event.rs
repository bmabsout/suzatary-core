//! A feed's events as occurrences: each `VEVENT` expanded through its `RRULE` and `RDATE`, less
//! its `EXDATE`, with each `RECURRENCE-ID` override standing in for the instance it names
//! (§3.8.5, §3.8.4.4), and only what falls in the window kept.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime};

use crate::content::{Component, Property};
use crate::expand;
use crate::item::{self, Item, MAX_ATTENDEES, MAX_NAME, MAX_TEXT, Status, When};
use crate::rule::{End, Rule};
use crate::value::{self, Moment};
use crate::zone::{self, Zone, Zones};

/// The span of UTC time an occurrence must touch to be emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The first instant, inclusive.
    pub from: NaiveDateTime,
    /// The last, exclusive.
    pub to: NaiveDateTime,
}

/// The most occurrences one calendar emits; past it the earliest are kept and the cut said.
pub const MAX_OCCURRENCES: usize = 2000;

/// What is said about a feed while reading it: a line for the operator, never an item.
pub type Said<'a> = &'a mut dyn FnMut(String);

/// Every occurrence of `calendar`'s events in `window`, named for `name`, sorted by UID and
/// occurrence.
pub fn occurrences(calendar: &Component, name: &str, window: Window, said: Said<'_>) -> Vec<Item> {
    let zones = Zones::of(calendar);
    let floating = calendar
        .first("X-WR-TIMEZONE")
        .and_then(|tz| zones.named(tz.value.trim()))
        .unwrap_or(Zone::Utc);
    let reader = Reader {
        name,
        zones: &zones,
        floating: &floating,
        window,
    };
    let mut masters: BTreeMap<String, &Component> = BTreeMap::new();
    let mut overrides: BTreeMap<String, Vec<&Component>> = BTreeMap::new();
    for event in calendar.children("VEVENT") {
        let Some(uid) = event.first("UID").map(|uid| uid.value.trim().to_owned()) else {
            said("an event with no UID was left out".to_owned());
            continue;
        };
        if event.first("RECURRENCE-ID").is_some() {
            overrides.entry(uid).or_default().push(event);
        } else if masters.insert(uid.clone(), event).is_some() {
            said(format!(
                "UID {uid} has two events and no RECURRENCE-ID: the last was read"
            ));
        }
    }
    let mut out: Vec<Item> = Vec::new();
    let uids: BTreeSet<&String> = masters.keys().chain(overrides.keys()).collect();
    for uid in uids {
        let master = masters.get(uid).copied();
        let mut replaced: BTreeMap<String, Item> = BTreeMap::new();
        for event in overrides.get(uid).into_iter().flatten() {
            if let Some((key, item)) = reader.instead(uid, event, master, said) {
                replaced.insert(key, item);
            }
        }
        if let Some(master) = master {
            for mut item in reader.instances(uid, master, said) {
                if let Some(instead) = replaced.remove(&item.occurrence) {
                    item = instead;
                }
                out.push(item);
            }
        }
        // An override whose instance the rule does not make (moved in, or made before an
        // EXDATE) is still an occurrence the feed states.
        out.extend(replaced.into_values());
    }
    out.retain(|item| item.overlaps(window.from, window.to));
    out.sort_by(|a, b| (&a.uid, &a.occurrence).cmp(&(&b.uid, &b.occurrence)));
    out.dedup_by(|a, b| a.uid == b.uid && a.occurrence == b.occurrence);
    if out.len() > MAX_OCCURRENCES {
        said(format!(
            "{} occurrences in the window; the earliest {MAX_OCCURRENCES} were kept",
            out.len()
        ));
        out.sort_by_key(|item| item.when.span().map(|(start, _)| start));
        out.truncate(MAX_OCCURRENCES);
        out.sort_by(|a, b| (&a.uid, &a.occurrence).cmp(&(&b.uid, &b.occurrence)));
    }
    out
}

/// How long an event lasts, as §3.8.5.3 applies it to every instance: `DTEND` minus `DTSTART` is
/// an EXACT duration, `DURATION` a nominal one.
#[derive(Debug, Clone, Copy)]
enum Length {
    Exact(Duration),
    Nominal(value::Duration),
}

struct Reader<'a> {
    name: &'a str,
    zones: &'a Zones,
    floating: &'a Zone,
    window: Window,
}

/// One instance before its fields are read: its identity and its span.
struct Instance {
    key: String,
    when: When,
}

impl Reader<'_> {
    fn zone_of(&self, moment: &Moment, said: Said<'_>) -> Zone {
        match moment {
            Moment::Zoned(_, tzid) => self.zones.named(tzid).unwrap_or_else(|| {
                said(format!(
                    "TZID {tzid} names no zone this plugin knows; read as UTC"
                ));
                Zone::Utc
            }),
            Moment::Floating(_) => self.floating.clone(),
            Moment::Utc(_) | Moment::Date(_) => Zone::Utc,
        }
    }

    /// An instant for a moment, reading an unknown zone as UTC.
    fn instant(&self, moment: &Moment, said: Said<'_>) -> Option<NaiveDateTime> {
        zone::instant(moment, self.zones, self.floating, &mut |tzid| {
            said(format!(
                "TZID {tzid} names no zone this plugin knows; read as UTC"
            ));
        })
    }

    /// The identity of the instance `moment` starts: a date, or a UTC instant.
    fn key(&self, moment: &Moment, said: Said<'_>) -> String {
        match moment {
            Moment::Date(date) => date.format("%Y%m%d").to_string(),
            other => self
                .instant(other, said)
                .map_or_else(String::new, |at| at.format("%Y%m%dT%H%M%SZ").to_string()),
        }
    }

    fn length(&self, event: &Component, start: &Moment, said: Said<'_>) -> Option<Length> {
        if let Some(end) = event.first("DTEND").and_then(value::of) {
            return Some(match (start, &end) {
                (Moment::Date(start), Moment::Date(end)) => Length::Nominal(value::Duration {
                    days: (*end - *start).num_days(),
                    seconds: 0,
                }),
                _ => Length::Exact(
                    self.instant(&end, said)
                        .zip(self.instant(start, said))
                        .map_or(Duration::zero(), |(end, start)| end - start),
                ),
            });
        }
        event
            .first("DURATION")
            .and_then(|duration| value::duration(&duration.value))
            .map(Length::Nominal)
    }

    /// The wall-clock starts its `RRULE` makes in the padded window, or its `DTSTART` alone.
    fn rule_starts(
        &self,
        uid: &str,
        event: &Component,
        start: &Moment,
        zone: &Zone,
        length: Option<Length>,
        said: Said<'_>,
    ) -> Vec<NaiveDateTime> {
        let all_day = matches!(start, Moment::Date(_));
        let local = start.naive();
        let tzid = match &start {
            Moment::Zoned(_, tzid) => Some(tzid.as_str()),
            _ => None,
        };
        // The window in this event's wall clock, padded by a day each side and by its length,
        // so an instance that began before the window and runs into it is found.
        let padding = Duration::days(2)
            + match length {
                Some(Length::Exact(duration)) => duration.abs(),
                Some(Length::Nominal(duration)) => {
                    Duration::days(duration.days.abs()) + Duration::seconds(duration.seconds.abs())
                }
                None => Duration::zero(),
            };
        let from = self.window.from - padding;
        let horizon = self.window.to + Duration::days(2);
        match event.first("RRULE") {
            None => vec![local],
            Some(rule) => match Rule::parse(&rule.value, tzid) {
                Err(refused) => {
                    said(format!(
                        "UID {uid}: RRULE not read ({}); its DTSTART alone stands",
                        refused.0
                    ));
                    vec![local]
                }
                Ok(rule) => {
                    let within = within_until(&rule, zone, all_day);
                    let made = expand::starts(&rule, local, &within, from, horizon);
                    if made.cut {
                        said(format!(
                            "UID {uid}: its rule was cut after {} periods",
                            expand::MAX_PERIODS
                        ));
                    }
                    made.starts
                }
            },
        }
    }

    /// Every instance of a master event in the window.
    fn instances(&self, uid: &str, event: &Component, said: Said<'_>) -> Vec<Item> {
        let Some(start) = event.first("DTSTART").and_then(value::of) else {
            said(format!("UID {uid} has no DTSTART and was left out"));
            return Vec::new();
        };
        let all_day = matches!(start, Moment::Date(_));
        let zone = self.zone_of(&start, said);
        let length = self.length(event, &start, said);
        let mut starts = self.rule_starts(uid, event, &start, &zone, length, said);
        if event.all("RRULE").count() > 1 {
            said(format!("UID {uid}: only its first RRULE was read"));
        }
        let mut extra: Vec<Instance> = Vec::new();
        for moment in event.all("RDATE").flat_map(value::all_of) {
            match (&moment, all_day) {
                (Moment::Date(day), true) => starts.push(day.and_time(NaiveTime::MIN)),
                (Moment::Date(_), false) | (_, true) => {
                    said(format!(
                        "UID {uid}: an RDATE of the other kind than its DTSTART was left out"
                    ));
                }
                (other, false) => {
                    if let Some(at) = self.instant(other, said) {
                        let instance_zone = Zone::Utc;
                        extra.push(Instance {
                            key: at.format("%Y%m%dT%H%M%SZ").to_string(),
                            when: span(false, at, &instance_zone, length),
                        });
                    }
                }
            }
        }
        let excluded: BTreeSet<String> = event
            .all("EXDATE")
            .flat_map(value::all_of)
            .map(|moment| self.key(&moment, said))
            .collect();
        let excluded_days: BTreeSet<NaiveDate> = event
            .all("EXDATE")
            .flat_map(value::all_of)
            .filter_map(|moment| match moment {
                Moment::Date(day) if !all_day => Some(day),
                _ => None,
            })
            .collect();
        let status = status(event);
        starts.sort_unstable();
        starts.dedup();
        starts
            .into_iter()
            .map(|local| {
                let when = span(all_day, local, &zone, length);
                let key = if all_day {
                    local.date().format("%Y%m%d").to_string()
                } else {
                    zone.to_utc(local).format("%Y%m%dT%H%M%SZ").to_string()
                };
                (local.date(), Instance { key, when })
            })
            .filter(|(day, _)| !excluded_days.contains(day))
            .map(|(_, instance)| instance)
            .chain(extra)
            .filter(|instance| !excluded.contains(&instance.key))
            .map(|instance| self.item(uid, instance, event, None, status))
            .collect()
    }

    /// The occurrence an override (an event with a `RECURRENCE-ID`) stands for, by the key of
    /// the instance it replaces.
    fn instead(
        &self,
        uid: &str,
        event: &Component,
        master: Option<&Component>,
        said: Said<'_>,
    ) -> Option<(String, Item)> {
        let recurrence = event.first("RECURRENCE-ID")?;
        if recurrence.param("RANGE").is_some() {
            said(format!(
                "UID {uid}: a RANGE on RECURRENCE-ID was read as this instance alone"
            ));
        }
        let original = value::of(recurrence)?;
        let key = self.key(&original, said);
        let start = event
            .first("DTSTART")
            .and_then(value::of)
            .unwrap_or_else(|| original.clone());
        let all_day = matches!(start, Moment::Date(_));
        let zone = self.zone_of(&start, said);
        let length = self.length(event, &start, said).or_else(|| {
            master.and_then(|master| {
                let first = master.first("DTSTART").and_then(value::of)?;
                self.length(master, &first, said)
            })
        });
        let when = span(all_day, start.naive(), &zone, length);
        let status = match (status(event), master.map(status)) {
            (_, Some(Status::Cancelled)) => Status::Cancelled,
            (own, _) => own,
        };
        let item = self.item(
            uid,
            Instance {
                key: key.clone(),
                when,
            },
            event,
            master,
            status,
        );
        Some((key, item))
    }

    fn item(
        &self,
        uid: &str,
        instance: Instance,
        event: &Component,
        master: Option<&Component>,
        status: Status,
    ) -> Item {
        let text = |name: &str| {
            event
                .first(name)
                .or_else(|| master.and_then(|master| master.first(name)))
                .map(|property| item::clipped(property.text().trim(), MAX_TEXT))
                .unwrap_or_default()
        };
        let attendees: Vec<String> = {
            let own: Vec<&Property> = event.all("ATTENDEE").collect();
            let from = if own.is_empty() {
                master
                    .map(|master| master.all("ATTENDEE").collect())
                    .unwrap_or_default()
            } else {
                own
            };
            from.into_iter()
                .filter_map(|attendee| attendee.param("CN"))
                .map(|name| item::clipped(crate::content::unescape(name).trim(), MAX_NAME))
                .filter(|name| !name.is_empty())
                .take(MAX_ATTENDEES)
                .collect()
        };
        Item {
            calendar: self.name.to_owned(),
            uid: item::clipped(uid, MAX_TEXT),
            occurrence: instance.key,
            title: text("SUMMARY"),
            when: instance.when,
            place: text("LOCATION"),
            attendees,
            status,
        }
    }
}

fn status(event: &Component) -> Status {
    match event
        .first("STATUS")
        .map(|status| status.value.trim().to_ascii_uppercase())
        .as_deref()
    {
        Some("CANCELLED") => Status::Cancelled,
        Some("TENTATIVE") => Status::Tentative,
        _ => Status::Confirmed,
    }
}

/// The span of an instance starting at the wall-clock `local` in `zone`; a date event's
/// `local` is its day's midnight.
fn span(all_day: bool, local: NaiveDateTime, zone: &Zone, length: Option<Length>) -> When {
    if all_day {
        let days = match length {
            Some(Length::Nominal(duration)) => duration.days.max(1),
            Some(Length::Exact(duration)) => duration.num_days().max(1),
            None => 1,
        };
        return When::all_day(local.date(), local.date() + Duration::days(days));
    }
    let start = zone.to_utc(local);
    let end = match length {
        None => start,
        Some(Length::Exact(duration)) => start + duration,
        Some(Length::Nominal(duration)) => {
            zone.to_utc(local + Duration::days(duration.days)) + Duration::seconds(duration.seconds)
        }
    };
    When::timed(start, end)
}

/// Whether a wall-clock instant is at or before the rule's `UNTIL`, which is UTC when it
/// ends in `Z` and the event's own clock otherwise.
fn within_until(rule: &Rule, zone: &Zone, all_day: bool) -> impl Fn(NaiveDateTime) -> bool + use<> {
    let until = match &rule.end {
        End::Until(until) => Some(until.clone()),
        End::Never | End::Count(_) => None,
    };
    let zone = zone.clone();
    move |local: NaiveDateTime| match &until {
        None => true,
        Some(Moment::Date(day)) => local.date() <= *day,
        Some(Moment::Utc(at)) if all_day => local.date() <= at.date(),
        Some(Moment::Utc(at)) => zone.to_utc(local) <= *at,
        Some(other) => local <= other.naive(),
    }
}

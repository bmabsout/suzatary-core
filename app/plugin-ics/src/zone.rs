//! Time zones: what a `TZID` names, and how a wall-clock time in it is placed in UTC.
//!
//! A `TZID` is read, in order, as an IANA name (chrono-tz's compiled database, Google's and
//! Proton's feeds), as the name of a `VTIMEZONE` the feed defines (Microsoft's "Pacific Standard
//! Time"), and as an IANA name inside a longer path (`/mozilla.org/20050126_1/Europe/London`).
//! A name none of these reads is read as UTC, and the plugin says so.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, NaiveDateTime, Offset, TimeZone};
use chrono_tz::Tz;

use crate::content::Component;
use crate::expand;
use crate::rule::Rule;
use crate::value::{self, Moment};

/// A zone a wall-clock time is read in.
#[derive(Debug, Clone)]
pub enum Zone {
    /// UTC, or what an unknown name falls back to.
    Utc,
    /// A zone of the IANA database.
    Iana(Tz),
    /// A zone the feed defines.
    Defined(Defined),
}

/// A `VTIMEZONE`: its observances (`STANDARD`, `DAYLIGHT`), each an offset taking effect at the
/// onsets its `DTSTART`, `RRULE` and `RDATE` make.
#[derive(Debug, Clone, Default)]
pub struct Defined {
    observances: Vec<Observance>,
}

#[derive(Debug, Clone)]
struct Observance {
    /// The first onset, in the wall clock of the offset it replaces.
    start: NaiveDateTime,
    from: i32,
    to: i32,
    rule: Option<Rule>,
    dates: Vec<NaiveDateTime>,
}

impl Observance {
    /// Where to start walking its rule for an instant in `utc`'s year. A yearly rule that counts
    /// nothing makes the same onsets from any year it is moved to, so it is moved to the year
    /// before, rather than walked from 1601 (Microsoft's first onset) on every reading. One with
    /// an `UNTIL` is walked from its start, since its first onset may be past its end.
    fn recent(&self, rule: &Rule, utc: NaiveDateTime) -> NaiveDateTime {
        let year = utc.year() - 1;
        let movable = rule.freq == crate::rule::Freq::Yearly
            && rule.interval == 1
            && rule.end == crate::rule::End::Never
            && self.start.year() < year;
        if movable {
            self.start.with_year(year).unwrap_or(self.start)
        } else {
            self.start
        }
    }
}

impl Defined {
    /// Read one `VTIMEZONE`; an observance missing a start or an offset is left out.
    #[must_use]
    pub fn of(component: &Component) -> Self {
        let observances = component
            .children
            .iter()
            .filter(|child| matches!(child.name.as_str(), "STANDARD" | "DAYLIGHT"))
            .filter_map(|child| {
                let start = child.first("DTSTART").and_then(value::of)?.naive();
                Some(Observance {
                    start,
                    from: value::offset(&child.first("TZOFFSETFROM")?.value)?,
                    to: value::offset(&child.first("TZOFFSETTO")?.value)?,
                    rule: child
                        .first("RRULE")
                        .and_then(|rule| Rule::parse(&rule.value, None).ok()),
                    dates: child
                        .all("RDATE")
                        .flat_map(value::all_of)
                        .map(|moment| moment.naive())
                        .collect(),
                })
            })
            .collect();
        Self { observances }
    }

    /// The offset in force at `utc`: the latest onset at or before it, or, before every onset,
    /// the offset the earliest one replaces.
    fn offset_at(&self, utc: NaiveDateTime) -> i32 {
        let mut latest: Option<(NaiveDateTime, i32)> = None;
        let mut earliest: Option<(NaiveDateTime, i32)> = None;
        for observance in &self.observances {
            let horizon =
                utc + Duration::seconds(i64::from(observance.from)) + Duration::seconds(1);
            let onsets = match &observance.rule {
                Some(rule) => {
                    let from = observance.from;
                    let until = match &rule.end {
                        crate::rule::End::Until(until) => Some(until.naive()),
                        _ => None,
                    };
                    // An observance's UNTIL is UTC (§3.6.5); its onsets are read in `from`.
                    let within = move |local: NaiveDateTime| {
                        until
                            .is_none_or(|until| local - Duration::seconds(i64::from(from)) <= until)
                    };
                    expand::starts(
                        rule,
                        observance.recent(rule, utc),
                        &within,
                        NaiveDateTime::MIN,
                        horizon,
                    )
                    .starts
                }
                None => vec![observance.start],
            };
            for local in onsets.into_iter().chain(observance.dates.iter().copied()) {
                let onset = local - Duration::seconds(i64::from(observance.from));
                if onset <= utc && latest.is_none_or(|(at, _)| onset > at) {
                    latest = Some((onset, observance.to));
                }
                if earliest.is_none_or(|(at, _)| onset < at) {
                    earliest = Some((onset, observance.from));
                }
            }
        }
        latest.or(earliest).map_or(0, |(_, offset)| offset)
    }
}

/// The feed's `VTIMEZONE`s by `TZID`.
#[derive(Debug, Clone, Default)]
pub struct Zones {
    defined: BTreeMap<String, Defined>,
}

impl Zones {
    /// Every `VTIMEZONE` in `calendar`.
    #[must_use]
    pub fn of(calendar: &Component) -> Self {
        Self {
            defined: calendar
                .children("VTIMEZONE")
                .filter_map(|zone| Some((zone.first("TZID")?.value.clone(), Defined::of(zone))))
                .collect(),
        }
    }

    /// The zone `tzid` names, or `None` for one nothing here reads.
    #[must_use]
    pub fn named(&self, tzid: &str) -> Option<Zone> {
        if let Ok(tz) = tzid.parse::<Tz>() {
            return Some(Zone::Iana(tz));
        }
        if let Some(defined) = self.defined.get(tzid) {
            return Some(Zone::Defined(defined.clone()));
        }
        tzid.match_indices('/')
            .find_map(|(at, _)| tzid[at + 1..].parse::<Tz>().ok())
            .map(Zone::Iana)
    }
}

impl Zone {
    /// The offset east of UTC in force at `utc`, in seconds.
    #[must_use]
    pub fn offset_at(&self, utc: NaiveDateTime) -> i32 {
        match self {
            Zone::Utc => 0,
            Zone::Iana(tz) => tz.offset_from_utc_datetime(&utc).fix().local_minus_utc(),
            Zone::Defined(defined) => defined.offset_at(utc),
        }
    }

    /// `local`, a reading of this zone's clock, in UTC.
    ///
    /// A reading the clock shows twice (the hour a change of offset repeats) is the FIRST, and
    /// one it skips (the hour it jumps over) is read with the offset in force before the jump,
    /// as §3.3.5 says of both.
    #[must_use]
    pub fn to_utc(&self, local: NaiveDateTime) -> NaiveDateTime {
        let before = self.offset_at(local - Duration::hours(36));
        let after = self.offset_at(local + Duration::hours(36));
        let mut valid: Vec<NaiveDateTime> = [before, after]
            .into_iter()
            .map(|offset| local - Duration::seconds(i64::from(offset)))
            .filter(|utc| {
                let offset = self.offset_at(*utc);
                *utc + Duration::seconds(i64::from(offset)) == local
            })
            .collect();
        valid.sort_unstable();
        valid
            .first()
            .copied()
            .unwrap_or_else(|| local - Duration::seconds(i64::from(before)))
    }
}

/// A [`Moment`]'s instant: a date-time placed in UTC through its zone, a floating one through
/// `floating` (the calendar's own zone, or UTC). `None` for a date, which has no instant, or for
/// a `TZID` nothing reads, which `unknown` is told of.
pub fn instant(
    moment: &Moment,
    zones: &Zones,
    floating: &Zone,
    unknown: &mut dyn FnMut(&str),
) -> Option<NaiveDateTime> {
    match moment {
        Moment::Date(_) => None,
        Moment::Utc(at) => Some(*at),
        Moment::Floating(at) => Some(floating.to_utc(*at)),
        Moment::Zoned(at, tzid) => Some(zones.named(tzid).map_or_else(
            || {
                unknown(tzid);
                *at
            },
            |zone| zone.to_utc(*at),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").expect("a time")
    }

    /// Microsoft's spelling of New York, with the United States' rules since 2007.
    const EASTERN: &str = "BEGIN:VCALENDAR\r\nBEGIN:VTIMEZONE\r\nTZID:Eastern Standard Time\r\nBEGIN:STANDARD\r\nDTSTART:16010101T020000\r\nTZOFFSETFROM:-0400\r\nTZOFFSETTO:-0500\r\nRRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=1SU;BYMONTH=11\r\nEND:STANDARD\r\nBEGIN:DAYLIGHT\r\nDTSTART:16010101T020000\r\nTZOFFSETFROM:-0500\r\nTZOFFSETTO:-0400\r\nRRULE:FREQ=YEARLY;INTERVAL=1;BYDAY=2SU;BYMONTH=3\r\nEND:DAYLIGHT\r\nEND:VTIMEZONE\r\nEND:VCALENDAR\r\n";

    fn eastern() -> Zone {
        let calendar = crate::content::parse(EASTERN).expect("a calendar");
        Zones::of(&calendar)
            .named("Eastern Standard Time")
            .expect("defined")
    }

    #[test]
    fn a_defined_zone_agrees_with_the_database_on_both_sides_of_each_change() {
        let iana = Zone::Iana(chrono_tz::America::New_York);
        let defined = eastern();
        for local in [
            "2026-01-15 09:00",
            "2026-03-07 23:00",
            "2026-03-08 03:30",
            "2026-07-04 12:00",
            "2026-10-31 23:00",
            "2026-11-01 03:00",
            "2026-12-31 23:59",
        ] {
            assert_eq!(defined.to_utc(at(local)), iana.to_utc(at(local)), "{local}");
        }
    }

    #[test]
    fn a_skipped_hour_is_read_with_the_offset_before_the_jump() {
        // 2:30 on 2026-03-08 does not exist in New York; read at -05:00 it is 7:30 UTC.
        assert_eq!(
            eastern().to_utc(at("2026-03-08 02:30")),
            at("2026-03-08 07:30")
        );
        assert_eq!(
            Zone::Iana(chrono_tz::America::New_York).to_utc(at("2026-03-08 02:30")),
            at("2026-03-08 07:30")
        );
    }

    #[test]
    fn a_repeated_hour_is_its_first_reading() {
        // 1:30 on 2026-11-01 happens twice in New York; the first is at -04:00, 5:30 UTC.
        assert_eq!(
            eastern().to_utc(at("2026-11-01 01:30")),
            at("2026-11-01 05:30")
        );
        assert_eq!(
            Zone::Iana(chrono_tz::America::New_York).to_utc(at("2026-11-01 01:30")),
            at("2026-11-01 05:30")
        );
    }

    #[test]
    fn a_name_inside_a_path_is_read_and_an_unknown_one_is_not() {
        let zones = Zones::default();
        assert!(matches!(
            zones.named("/mozilla.org/20050126_1/Europe/London"),
            Some(Zone::Iana(chrono_tz::Europe::London))
        ));
        assert!(zones.named("Somewhere Standard Time").is_none());
    }
}

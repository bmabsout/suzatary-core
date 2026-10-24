//! RFC 5545 §3.3: the value types a calendar's times are written in.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

use crate::content::Property;

/// A DATE or DATE-TIME as written, before any zone is applied (§3.3.4, §3.3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Moment {
    /// A whole day.
    Date(NaiveDate),
    /// Wall-clock time with no zone: the same clock reading wherever it is read (form 1).
    Floating(NaiveDateTime),
    /// UTC, written with a `Z` (form 2).
    Utc(NaiveDateTime),
    /// Wall-clock time in the zone a `TZID` names (form 3).
    Zoned(NaiveDateTime, String),
}

impl Moment {
    /// The clock reading, whatever zone it is in; a date reads as its midnight.
    #[must_use]
    pub fn naive(&self) -> NaiveDateTime {
        match self {
            Moment::Date(date) => date.and_time(NaiveTime::MIN),
            Moment::Floating(at) | Moment::Utc(at) | Moment::Zoned(at, _) => *at,
        }
    }
}

/// One DATE or DATE-TIME text, read with `tzid` for a local time.
#[must_use]
pub fn moment(text: &str, date_only: bool, tzid: Option<&str>) -> Option<Moment> {
    let text = text.trim();
    if date_only || text.len() == 8 {
        return NaiveDate::parse_from_str(text, "%Y%m%d")
            .ok()
            .map(Moment::Date);
    }
    if let Some(utc) = text.strip_suffix(['Z', 'z']) {
        return NaiveDateTime::parse_from_str(utc, "%Y%m%dT%H%M%S")
            .ok()
            .map(Moment::Utc);
    }
    let local = NaiveDateTime::parse_from_str(text, "%Y%m%dT%H%M%S").ok()?;
    Some(match tzid {
        Some(tzid) => Moment::Zoned(local, tzid.to_owned()),
        None => Moment::Floating(local),
    })
}

/// A property whose value is one DATE or DATE-TIME (`DTSTART`, `DTEND`, `RECURRENCE-ID`).
#[must_use]
pub fn of(property: &Property) -> Option<Moment> {
    moment(
        &property.value,
        property.param("VALUE") == Some("DATE"),
        property.param("TZID"),
    )
}

/// A property whose value is a list of them (`EXDATE`, `RDATE`); a PERIOD (§3.3.9) counts by
/// its start.
#[must_use]
pub fn all_of(property: &Property) -> Vec<Moment> {
    let date_only = property.param("VALUE") == Some("DATE");
    crate::content::list(&property.value)
        .into_iter()
        .filter_map(|item| {
            let start = item.split_once('/').map_or(item, |(start, _)| start);
            moment(start, date_only, property.param("TZID"))
        })
        .collect()
}

/// A DURATION (§3.3.6), split the way §3.8.5.3 applies it: whole days are NOMINAL (a day is a
/// calendar day, 23 or 25 hours across a change of offset) and the rest is EXACT seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Duration {
    /// Days, weeks counted as seven.
    pub days: i64,
    /// Hours, minutes and seconds, in seconds.
    pub seconds: i64,
}

/// `[+-]P[nW]` or `[+-]P[nD][T[nH][nM][nS]]`.
#[must_use]
pub fn duration(text: &str) -> Option<Duration> {
    let text = text.trim();
    let (sign, rest) = match text.as_bytes().first()? {
        b'-' => (-1, &text[1..]),
        b'+' => (1, &text[1..]),
        _ => (1, text),
    };
    let rest = rest.strip_prefix('P')?;
    let (date, time) = rest.split_once('T').unwrap_or((rest, ""));
    let mut out = Duration::default();
    let mut number = String::new();
    for character in date.chars() {
        match character {
            '0'..='9' => number.push(character),
            'W' => out.days += 7 * number.parse::<i64>().ok()?,
            'D' => out.days += number.parse::<i64>().ok()?,
            _ => return None,
        }
        if !character.is_ascii_digit() {
            number.clear();
        }
    }
    if !number.is_empty() {
        return None;
    }
    for character in time.chars() {
        match character {
            '0'..='9' => number.push(character),
            'H' => out.seconds += 3600 * number.parse::<i64>().ok()?,
            'M' => out.seconds += 60 * number.parse::<i64>().ok()?,
            'S' => out.seconds += number.parse::<i64>().ok()?,
            _ => return None,
        }
        if !character.is_ascii_digit() {
            number.clear();
        }
    }
    if !number.is_empty() || (date.is_empty() && time.is_empty()) {
        return None;
    }
    Some(Duration {
        days: sign * out.days,
        seconds: sign * out.seconds,
    })
}

/// A UTC-OFFSET (§3.3.14), `+HHMM` or `-HHMMSS`, in seconds east of UTC.
#[must_use]
pub fn offset(text: &str) -> Option<i32> {
    let text = text.trim();
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let digits = &text[1..];
    if !(digits.len() == 4 || digits.len() == 6) || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let field =
        |range: std::ops::Range<usize>| digits.get(range).and_then(|d| d.parse::<i32>().ok());
    let seconds = field(0..2)? * 3600 + field(2..4)? * 60 + field(4..6).unwrap_or(0);
    Some(sign * seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_keeps_its_days_nominal_and_its_hours_exact() {
        assert_eq!(
            duration("PT1H30M"),
            Some(Duration {
                days: 0,
                seconds: 5400
            })
        );
        assert_eq!(
            duration("P1D"),
            Some(Duration {
                days: 1,
                seconds: 0
            })
        );
        assert_eq!(
            duration("P2W"),
            Some(Duration {
                days: 14,
                seconds: 0
            })
        );
        assert_eq!(
            duration("-P1DT2H"),
            Some(Duration {
                days: -1,
                seconds: -7200
            })
        );
        assert_eq!(duration("P"), None);
        assert_eq!(duration("PT5"), None);
        assert_eq!(duration("1H"), None);
    }
}

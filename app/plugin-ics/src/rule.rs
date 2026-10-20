//! RFC 5545 §3.3.10: a recurrence rule, parsed once into a value that holds only what the RFC
//! allows together.

use chrono::Weekday;

use crate::value::{Moment, moment};

/// How often the rule's periods come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Freq {
    /// Every second.
    Secondly,
    /// Every minute.
    Minutely,
    /// Every hour.
    Hourly,
    /// Every day.
    Daily,
    /// Every week, from `WKST`.
    Weekly,
    /// Every month.
    Monthly,
    /// Every year.
    Yearly,
}

/// When the recurrence stops: `COUNT` and `UNTIL` together are not a rule (§3.3.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum End {
    /// Never: the reader's horizon stops it.
    Never,
    /// After this many instances, `DTSTART` counted first.
    Count(u32),
    /// At this moment, inclusive.
    Until(Moment),
}

/// A weekday, with an ordinal for "the second Monday" or "the last Friday" (`2MO`, `-1FR`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Day {
    /// Which of the period's such weekdays, from the end when negative; `None` for every one.
    pub nth: Option<i32>,
    /// The weekday.
    pub weekday: Weekday,
}

/// One rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// `FREQ`.
    pub freq: Freq,
    /// `INTERVAL`, at least one.
    pub interval: u32,
    /// `COUNT` or `UNTIL`.
    pub end: End,
    /// `BYSECOND`.
    pub by_second: Vec<u32>,
    /// `BYMINUTE`.
    pub by_minute: Vec<u32>,
    /// `BYHOUR`.
    pub by_hour: Vec<u32>,
    /// `BYDAY`.
    pub by_day: Vec<Day>,
    /// `BYMONTHDAY`, from the end when negative.
    pub by_month_day: Vec<i32>,
    /// `BYYEARDAY`, from the end when negative.
    pub by_year_day: Vec<i32>,
    /// `BYWEEKNO`, from the end when negative.
    pub by_week_no: Vec<i32>,
    /// `BYMONTH`.
    pub by_month: Vec<u32>,
    /// `BYSETPOS`, from the end when negative.
    pub by_set_pos: Vec<i32>,
    /// `WKST`, Monday unless said.
    pub week_start: Weekday,
}

/// Why a rule was not read; the event is then read as its `DTSTART` alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadRule(pub String);

impl Rule {
    /// Parse an `RRULE` value; an `UNTIL` local time is read in `tzid`, as its `DTSTART` is.
    ///
    /// # Errors
    /// A part this reader does not know, a value out of its range, or two parts the RFC forbids
    /// together.
    pub fn parse(text: &str, tzid: Option<&str>) -> Result<Self, BadRule> {
        let bad = |why: String| BadRule(why);
        let mut freq = None;
        let mut rule = Rule {
            freq: Freq::Daily,
            interval: 1,
            end: End::Never,
            by_second: Vec::new(),
            by_minute: Vec::new(),
            by_hour: Vec::new(),
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_year_day: Vec::new(),
            by_week_no: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
            week_start: Weekday::Mon,
        };
        let mut count = None;
        let mut until = None;
        for part in text.trim().split(';').filter(|part| !part.is_empty()) {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| bad(format!("{part:?} is not KEY=VALUE")))?;
            match key.to_ascii_uppercase().as_str() {
                "FREQ" => {
                    freq = Some(frequency(value).ok_or_else(|| bad(format!("FREQ={value}")))?);
                }
                "INTERVAL" => {
                    rule.interval = value
                        .parse()
                        .ok()
                        .filter(|n| *n >= 1)
                        .ok_or_else(|| bad(format!("INTERVAL={value}")))?;
                }
                "COUNT" => {
                    count = Some(
                        value
                            .parse::<u32>()
                            .map_err(|_| bad(format!("COUNT={value}")))?,
                    );
                }
                "UNTIL" => {
                    until = Some(
                        moment(value, false, tzid).ok_or_else(|| bad(format!("UNTIL={value}")))?,
                    );
                }
                "BYSECOND" => rule.by_second = numbers(value, 0, 60, key)?,
                "BYMINUTE" => rule.by_minute = numbers(value, 0, 59, key)?,
                "BYHOUR" => rule.by_hour = numbers(value, 0, 23, key)?,
                "BYMONTH" => rule.by_month = numbers(value, 1, 12, key)?,
                "BYMONTHDAY" => rule.by_month_day = signed(value, 31, key)?,
                "BYYEARDAY" => rule.by_year_day = signed(value, 366, key)?,
                "BYWEEKNO" => rule.by_week_no = signed(value, 53, key)?,
                "BYSETPOS" => rule.by_set_pos = signed(value, 366, key)?,
                "BYDAY" => {
                    rule.by_day = value
                        .split(',')
                        .map(|day| {
                            weekday_with_ordinal(day).ok_or_else(|| bad(format!("BYDAY {day}")))
                        })
                        .collect::<Result<_, _>>()?;
                }
                "WKST" => {
                    rule.week_start = weekday(value).ok_or_else(|| bad(format!("WKST={value}")))?;
                }
                other => return Err(bad(format!("{other} is not a rule part this reader knows"))),
            }
        }
        rule.freq = freq.ok_or_else(|| bad("no FREQ".to_owned()))?;
        rule.end = match (count, until) {
            (Some(_), Some(_)) => return Err(bad("COUNT and UNTIL together".to_owned())),
            (Some(count), None) => End::Count(count),
            (None, Some(until)) => End::Until(until),
            (None, None) => End::Never,
        };
        let ordinal = rule.by_day.iter().any(|day| day.nth.is_some());
        if ordinal && !matches!(rule.freq, Freq::Monthly | Freq::Yearly) {
            return Err(bad("an ordinal BYDAY outside MONTHLY or YEARLY".to_owned()));
        }
        if ordinal && rule.freq == Freq::Yearly && !rule.by_week_no.is_empty() {
            return Err(bad("an ordinal BYDAY with BYWEEKNO".to_owned()));
        }
        if !rule.by_month_day.is_empty() && rule.freq == Freq::Weekly {
            return Err(bad("BYMONTHDAY with FREQ=WEEKLY".to_owned()));
        }
        if !rule.by_year_day.is_empty()
            && matches!(rule.freq, Freq::Daily | Freq::Weekly | Freq::Monthly)
        {
            return Err(bad(
                "BYYEARDAY with FREQ=DAILY, WEEKLY or MONTHLY".to_owned()
            ));
        }
        if !rule.by_week_no.is_empty() && rule.freq != Freq::Yearly {
            return Err(bad("BYWEEKNO outside FREQ=YEARLY".to_owned()));
        }
        Ok(rule)
    }
}

fn frequency(text: &str) -> Option<Freq> {
    Some(match text.to_ascii_uppercase().as_str() {
        "SECONDLY" => Freq::Secondly,
        "MINUTELY" => Freq::Minutely,
        "HOURLY" => Freq::Hourly,
        "DAILY" => Freq::Daily,
        "WEEKLY" => Freq::Weekly,
        "MONTHLY" => Freq::Monthly,
        "YEARLY" => Freq::Yearly,
        _ => return None,
    })
}

/// `MO`…`SU`.
#[must_use]
pub fn weekday(text: &str) -> Option<Weekday> {
    Some(match text.trim().to_ascii_uppercase().as_str() {
        "MO" => Weekday::Mon,
        "TU" => Weekday::Tue,
        "WE" => Weekday::Wed,
        "TH" => Weekday::Thu,
        "FR" => Weekday::Fri,
        "SA" => Weekday::Sat,
        "SU" => Weekday::Sun,
        _ => return None,
    })
}

fn weekday_with_ordinal(text: &str) -> Option<Day> {
    let text = text.trim();
    let split = text.len().checked_sub(2)?;
    let (nth, day) = text.split_at(split);
    let nth = if nth.is_empty() {
        None
    } else {
        let n: i32 = nth.trim_start_matches('+').parse().ok()?;
        (n != 0 && n.abs() <= 53).then_some(n)?;
        Some(n)
    };
    Some(Day {
        nth,
        weekday: weekday(day)?,
    })
}

fn numbers(text: &str, low: u32, high: u32, key: &str) -> Result<Vec<u32>, BadRule> {
    text.split(',')
        .map(|n| {
            n.trim()
                .parse::<u32>()
                .ok()
                .filter(|n| (low..=high).contains(n))
                .ok_or_else(|| BadRule(format!("{key}={text}")))
        })
        .collect()
}

fn signed(text: &str, high: i32, key: &str) -> Result<Vec<i32>, BadRule> {
    text.split(',')
        .map(|n| {
            n.trim()
                .trim_start_matches('+')
                .parse::<i32>()
                .ok()
                .filter(|n| *n != 0 && n.abs() <= high)
                .ok_or_else(|| BadRule(format!("{key}={text}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rule_reads_every_part() {
        let rule = Rule::parse(
            "FREQ=MONTHLY;INTERVAL=2;BYDAY=2MO,-1FR;COUNT=10;WKST=SU",
            None,
        )
        .expect("a rule");
        assert_eq!(rule.freq, Freq::Monthly);
        assert_eq!(rule.interval, 2);
        assert_eq!(rule.end, End::Count(10));
        assert_eq!(
            rule.by_day,
            [
                Day {
                    nth: Some(2),
                    weekday: Weekday::Mon
                },
                Day {
                    nth: Some(-1),
                    weekday: Weekday::Fri
                }
            ]
        );
        assert_eq!(rule.week_start, Weekday::Sun);
    }

    #[test]
    fn what_the_rfc_forbids_together_is_refused() {
        for text in [
            "FREQ=DAILY;COUNT=2;UNTIL=20261231T000000Z",
            "FREQ=WEEKLY;BYDAY=2MO",
            "FREQ=WEEKLY;BYMONTHDAY=3",
            "FREQ=MONTHLY;BYYEARDAY=100",
            "FREQ=MONTHLY;BYWEEKNO=2",
            "FREQ=YEARLY;BYWEEKNO=2;BYDAY=1MO",
            "INTERVAL=2",
            "FREQ=DAILY;INTERVAL=0",
            "FREQ=DAILY;BYMONTHDAY=32",
            "FREQ=DAILY;BYHOUR=24",
            "FREQ=DAILY;BYDAY=XX",
            "FREQ=DAILY;X-NAME=1",
        ] {
            assert!(Rule::parse(text, None).is_err(), "{text}");
        }
    }

    #[test]
    fn until_is_read_in_the_zone_dtstart_is() {
        let rule =
            Rule::parse("FREQ=DAILY;UNTIL=20261010T090000", Some("Europe/Paris")).expect("a rule");
        assert!(
            matches!(rule.end, End::Until(Moment::Zoned(_, ref zone)) if zone == "Europe/Paris")
        );
    }
}

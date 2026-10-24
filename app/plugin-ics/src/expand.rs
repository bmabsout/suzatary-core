//! RFC 5545 §3.3.10 and §3.8.5.3: the instances a rule makes, in wall-clock time.
//!
//! A rule is expanded in the zone its `DTSTART` is written in, as a reading of the clock, and
//! each instance is placed in UTC afterwards (`crate::zone`): "every day at 9" stays at 9 across
//! a change of offset. The rule's periods are walked from `DTSTART`, because `COUNT` counts from
//! there, and only the instances in `[from, horizon)` are kept.

use chrono::{
    Datelike, Days, Duration, Months, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Weekday,
};

use crate::rule::{Day, End, Freq, Rule};

/// The most periods walked for one rule. A daily rule begun fifty years ago is under twenty
/// thousand; past this the rule is cut and the cut said.
pub const MAX_PERIODS: u32 = 200_000;

/// What one rule made inside the window.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Expansion {
    /// Each instance's start, ascending, `DTSTART` first when it is in the window.
    pub starts: Vec<NaiveDateTime>,
    /// Whether [`MAX_PERIODS`] stopped the walk before the horizon did.
    pub cut: bool,
}

/// The instances of `rule` from `dtstart` that start in `[from, horizon)`.
///
/// `DTSTART` is always the first instance, and counts toward `COUNT`, whether or not the rule
/// would make it (§3.3.10: "The DTSTART property value always counts as the first
/// occurrence"). `within_until` says whether an instance is at or before the rule's `UNTIL`,
/// which may be in another zone than the wall clock the rule runs on.
#[must_use]
pub fn starts(
    rule: &Rule,
    dtstart: NaiveDateTime,
    within_until: &dyn Fn(NaiveDateTime) -> bool,
    from: NaiveDateTime,
    horizon: NaiveDateTime,
) -> Expansion {
    let limit = match rule.end {
        End::Count(0) => return Expansion::default(),
        End::Count(count) => Some(count),
        End::Never | End::Until(_) => None,
    };
    let rule = &defaulted(rule, dtstart);
    let mut out = Expansion::default();
    let mut made: u32 = 1;
    let keep = |at: NaiveDateTime, out: &mut Expansion| {
        if at >= from && at < horizon {
            out.starts.push(at);
        }
    };
    keep(dtstart, &mut out);
    let mut last = dtstart;
    for k in 0..MAX_PERIODS {
        if limit.is_some_and(|limit| made >= limit) {
            return out;
        }
        let Some(period) = Period::nth(rule, dtstart, k) else {
            return out;
        };
        if period.begins() >= horizon {
            return out;
        }
        for at in instances(rule, &period) {
            if at <= last {
                continue;
            }
            if at >= horizon || !within_until(at) || limit.is_some_and(|limit| made >= limit) {
                return out;
            }
            keep(at, &mut out);
            last = at;
            made += 1;
        }
    }
    out.cut = true;
    out
}

/// §3.3.10's defaults: a rule that names no day takes its day from `DTSTART`, and a coarse rule
/// that names no time takes `DTSTART`'s.
fn defaulted(rule: &Rule, dtstart: NaiveDateTime) -> Rule {
    let mut rule = rule.clone();
    let names_a_day = !(rule.by_week_no.is_empty()
        && rule.by_year_day.is_empty()
        && rule.by_month_day.is_empty()
        && rule.by_day.is_empty());
    if !names_a_day {
        match rule.freq {
            Freq::Yearly => {
                if rule.by_month.is_empty() {
                    rule.by_month = vec![dtstart.month()];
                }
                rule.by_month_day = vec![day_of(dtstart)];
            }
            Freq::Monthly => rule.by_month_day = vec![day_of(dtstart)],
            Freq::Weekly => {
                rule.by_day = vec![Day {
                    nth: None,
                    weekday: dtstart.weekday(),
                }];
            }
            Freq::Daily | Freq::Hourly | Freq::Minutely | Freq::Secondly => {}
        }
    }
    if rule.freq > Freq::Hourly && rule.by_hour.is_empty() {
        rule.by_hour = vec![dtstart.hour()];
    }
    if rule.freq > Freq::Minutely && rule.by_minute.is_empty() {
        rule.by_minute = vec![dtstart.minute()];
    }
    if rule.freq > Freq::Secondly && rule.by_second.is_empty() {
        rule.by_second = vec![dtstart.second()];
    }
    rule
}

fn day_of(at: NaiveDateTime) -> i32 {
    i32::try_from(at.day()).unwrap_or(1)
}

/// One period of the rule's frequency.
enum Period {
    /// A year, by its number; with `BYWEEKNO`, its week-year.
    Year(i32),
    /// A month of a year.
    Month(i32, u32),
    /// Seven days from the one `WKST` names.
    Week(NaiveDate),
    /// One day.
    Day(NaiveDate),
    /// One hour, minute or second, by its start.
    Instant(NaiveDateTime),
}

impl Period {
    /// The `k`th period from the one holding `dtstart`, `INTERVAL` apart; `None` past the
    /// calendar's end.
    fn nth(rule: &Rule, dtstart: NaiveDateTime, k: u32) -> Option<Self> {
        let step = k.checked_mul(rule.interval)?;
        let date = dtstart.date();
        Some(match rule.freq {
            Freq::Yearly => Period::Year(dtstart.year().checked_add(i32::try_from(step).ok()?)?),
            Freq::Monthly => {
                let first = NaiveDate::from_ymd_opt(date.year(), date.month(), 1)?;
                let month = first.checked_add_months(Months::new(step))?;
                Period::Month(month.year(), month.month())
            }
            Freq::Weekly => {
                let start = date.checked_sub_days(Days::new(u64::from(since(
                    date.weekday(),
                    rule.week_start,
                ))))?;
                Period::Week(start.checked_add_days(Days::new(u64::from(step) * 7))?)
            }
            Freq::Daily => Period::Day(date.checked_add_days(Days::new(u64::from(step)))?),
            Freq::Hourly => Period::Instant(
                dtstart.with_minute(0)?.with_second(0)? + Duration::hours(i64::from(step)),
            ),
            Freq::Minutely => {
                Period::Instant(dtstart.with_second(0)? + Duration::minutes(i64::from(step)))
            }
            Freq::Secondly => Period::Instant(dtstart + Duration::seconds(i64::from(step))),
        })
    }

    /// The first moment in it.
    fn begins(&self) -> NaiveDateTime {
        match self {
            Period::Year(year) => NaiveDate::from_ymd_opt(*year, 1, 1)
                .map_or(NaiveDateTime::MAX, |day| {
                    day.and_time(NaiveTime::MIN) - Duration::days(7)
                }),
            Period::Month(year, month) => NaiveDate::from_ymd_opt(*year, *month, 1)
                .map_or(NaiveDateTime::MAX, |day| day.and_time(NaiveTime::MIN)),
            Period::Week(day) | Period::Day(day) => day.and_time(NaiveTime::MIN),
            Period::Instant(at) => *at,
        }
    }
}

/// Days from `start` forward to `day`, 0 to 6.
fn since(day: Weekday, start: Weekday) -> u32 {
    (7 + day.num_days_from_monday() - start.num_days_from_monday()) % 7
}

/// The first day of week 1 of `year` under `start`: the week holding the 4th of January, which
/// is the first with four days in the year.
fn week_one(year: i32, start: Weekday) -> Option<NaiveDate> {
    let fourth = NaiveDate::from_ymd_opt(year, 1, 4)?;
    fourth.checked_sub_days(Days::new(u64::from(since(fourth.weekday(), start))))
}

/// Every day from `first` to `last`, inclusive.
fn days(first: NaiveDate, last: NaiveDate) -> Vec<NaiveDate> {
    first.iter_days().take_while(|day| *day <= last).collect()
}

fn last_of_month(year: i32, month: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(year, month, 1)?
        .checked_add_months(Months::new(1))?
        .pred_opt()
}

/// The period's instances, sorted, `BYSETPOS` applied.
fn instances(rule: &Rule, period: &Period) -> Vec<NaiveDateTime> {
    let candidates: Vec<NaiveDate> = match period {
        Period::Year(year) if !rule.by_week_no.is_empty() => {
            match (
                week_one(*year, rule.week_start),
                week_one(year + 1, rule.week_start),
            ) {
                (Some(first), Some(next)) => next
                    .pred_opt()
                    .map_or_else(Vec::new, |last| days(first, last)),
                _ => Vec::new(),
            }
        }
        Period::Year(year) => match (
            NaiveDate::from_ymd_opt(*year, 1, 1),
            NaiveDate::from_ymd_opt(*year, 12, 31),
        ) {
            (Some(first), Some(last)) => days(first, last),
            _ => Vec::new(),
        },
        Period::Month(year, month) => match (
            NaiveDate::from_ymd_opt(*year, *month, 1),
            last_of_month(*year, *month),
        ) {
            (Some(first), Some(last)) => days(first, last),
            _ => Vec::new(),
        },
        Period::Week(first) => first
            .checked_add_days(Days::new(6))
            .map_or_else(Vec::new, |last| days(*first, last)),
        Period::Day(day) => vec![*day],
        Period::Instant(at) => vec![at.date()],
    };
    let mut set: Vec<NaiveDateTime> = candidates
        .into_iter()
        .filter(|day| on_a_named_day(rule, period, *day))
        .flat_map(|day| {
            times(rule, period)
                .into_iter()
                .map(move |time| day.and_time(time))
        })
        .collect();
    set.sort_unstable();
    set.dedup();
    if rule.by_set_pos.is_empty() {
        return set;
    }
    let len = i32::try_from(set.len()).unwrap_or(i32::MAX);
    let mut picked: Vec<NaiveDateTime> = rule
        .by_set_pos
        .iter()
        .filter_map(|&pos| {
            let index = if pos > 0 { pos - 1 } else { len + pos };
            usize::try_from(index)
                .ok()
                .and_then(|index| set.get(index).copied())
        })
        .collect();
    picked.sort_unstable();
    picked.dedup();
    picked
}

/// Whether `day` passes every day-level part: for a coarse rule a part EXPANDS the period to the
/// days it names, which is the same as keeping the period's days it names.
fn on_a_named_day(rule: &Rule, period: &Period, day: NaiveDate) -> bool {
    let in_month = rule.by_month.is_empty() || rule.by_month.contains(&day.month());
    let in_week = rule.by_week_no.is_empty() || {
        let Period::Year(year) = period else {
            return false;
        };
        week_number(*year, rule.week_start, day).is_some_and(|(number, weeks)| {
            rule.by_week_no
                .iter()
                .any(|&n| n == number || n == number - weeks - 1)
        })
    };
    let in_year_day = rule.by_year_day.is_empty() || {
        let ordinal = i32::try_from(day.ordinal()).unwrap_or(0);
        let length = if day.leap_year() { 366 } else { 365 };
        rule.by_year_day
            .iter()
            .any(|&n| n == ordinal || n == ordinal - length - 1)
    };
    let in_month_day = rule.by_month_day.is_empty() || {
        let date = i32::try_from(day.day()).unwrap_or(0);
        let length = last_of_month(day.year(), day.month())
            .map_or(31, |last| i32::try_from(last.day()).unwrap_or(31));
        rule.by_month_day
            .iter()
            .any(|&n| n == date || n == date - length - 1)
    };
    let on_weekday = rule.by_day.is_empty()
        || rule.by_day.iter().any(|named| {
            named.weekday == day.weekday()
                && named.nth.is_none_or(|nth| {
                    let (first, last) = frame(rule, day);
                    let from_start = i32::try_from((day - first).num_days() / 7).unwrap_or(0) + 1;
                    let from_end = -(i32::try_from((last - day).num_days() / 7).unwrap_or(0) + 1);
                    nth == from_start || nth == from_end
                })
        });
    in_month && in_week && in_year_day && in_month_day && on_weekday
}

/// What an ordinal `BYDAY` counts within: the month for a monthly rule or a yearly one that
/// names months, else the year.
fn frame(rule: &Rule, day: NaiveDate) -> (NaiveDate, NaiveDate) {
    let month = || {
        (
            day.with_day(1).unwrap_or(day),
            last_of_month(day.year(), day.month()).unwrap_or(day),
        )
    };
    match rule.freq {
        Freq::Yearly if rule.by_month.is_empty() => (
            NaiveDate::from_ymd_opt(day.year(), 1, 1).unwrap_or(day),
            NaiveDate::from_ymd_opt(day.year(), 12, 31).unwrap_or(day),
        ),
        _ => month(),
    }
}

/// `day`'s week number within `year`'s week-year, and how many weeks that week-year has.
fn week_number(year: i32, start: Weekday, day: NaiveDate) -> Option<(i32, i32)> {
    let first = week_one(year, start)?;
    let next = week_one(year + 1, start)?;
    if day < first || day >= next {
        return None;
    }
    let number = i32::try_from((day - first).num_days() / 7).ok()? + 1;
    let weeks = i32::try_from((next - first).num_days() / 7).ok()?;
    Some((number, weeks))
}

/// The times of day an instance may start at in this period: a coarse rule's `BYHOUR`,
/// `BYMINUTE` and `BYSECOND` expand, and a fine one's own hour, minute or second must be named
/// by them where they are given.
fn times(rule: &Rule, period: &Period) -> Vec<NaiveTime> {
    let own = match period {
        Period::Instant(at) => Some(at.time()),
        _ => None,
    };
    let pick = |named: &[u32], fixed: Option<u32>| -> Vec<u32> {
        match fixed {
            Some(value) if named.is_empty() || named.contains(&value) => vec![value],
            Some(_) => Vec::new(),
            None => named.to_vec(),
        }
    };
    let hour = own.filter(|_| rule.freq <= Freq::Hourly).map(|t| t.hour());
    let minute = own
        .filter(|_| rule.freq <= Freq::Minutely)
        .map(|t| t.minute());
    let second = own
        .filter(|_| rule.freq <= Freq::Secondly)
        .map(|t| t.second());
    let mut out = Vec::new();
    for h in pick(&rule.by_hour, hour) {
        for m in pick(&rule.by_minute, minute) {
            for s in pick(&rule.by_second, second) {
                // A leap second (60) is no time a clock here reads, so it makes no instance.
                if let Some(time) = NaiveTime::from_hms_opt(h, m, s) {
                    out.push(time);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M").expect("a time")
    }

    /// Every instance from `dtstart` until 2030, as `YYYY-MM-DD HH:MM`.
    fn expand(rule: &str, dtstart: &str) -> Vec<String> {
        let rule = Rule::parse(rule, None).expect("a rule");
        let until = match &rule.end {
            End::Until(until) => Some(until.naive()),
            _ => None,
        };
        starts(
            &rule,
            at(dtstart),
            &|instance| until.is_none_or(|until| instance <= until),
            at("1900-01-01 00:00"),
            at("2030-01-01 00:00"),
        )
        .starts
        .into_iter()
        .map(|instance| instance.format("%Y-%m-%d %H:%M").to_string())
        .collect()
    }

    /// The RFC's own examples (§3.8.5.3 and §3.3.10's two week starts), each against the list it
    /// prints, and the cases its prose names: a day a month or a year does not have is skipped,
    /// never rolled; week one may begin in December; `DTSTART` is the first instance even when
    /// the rule would not make it. Each row is a rule, its `DTSTART`, how many instances it makes
    /// until 2030, and runs of them from a position.
    #[test]
    #[allow(clippy::too_many_lines, reason = "one row per example the RFC prints")]
    fn each_rule_makes_the_instances_the_rfc_prints() {
        type Runs<'a> = &'a [(usize, &'a [&'a str])];
        let rows: &[(&str, &str, usize, Runs<'_>)] = &[
            (
                "FREQ=DAILY;COUNT=10",
                "1997-09-02 09:00",
                10,
                &[(9, &["1997-09-11 09:00"])],
            ),
            (
                "FREQ=DAILY;INTERVAL=2;UNTIL=19970912T090000",
                "1997-09-02 09:00",
                6,
                &[(
                    0,
                    &[
                        "1997-09-02 09:00",
                        "1997-09-04 09:00",
                        "1997-09-06 09:00",
                        "1997-09-08 09:00",
                        "1997-09-10 09:00",
                        "1997-09-12 09:00",
                    ],
                )],
            ),
            (
                "FREQ=WEEKLY;UNTIL=19971007T000000;WKST=SU;BYDAY=TU,TH",
                "1997-09-02 09:00",
                10,
                &[
                    (
                        0,
                        &["1997-09-02 09:00", "1997-09-04 09:00", "1997-09-09 09:00"],
                    ),
                    (9, &["1997-10-02 09:00"]),
                ],
            ),
            (
                "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=MO",
                "1997-08-05 09:00",
                4,
                &[(
                    0,
                    &[
                        "1997-08-05 09:00",
                        "1997-08-10 09:00",
                        "1997-08-19 09:00",
                        "1997-08-24 09:00",
                    ],
                )],
            ),
            (
                "FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=SU",
                "1997-08-05 09:00",
                4,
                &[(
                    0,
                    &[
                        "1997-08-05 09:00",
                        "1997-08-17 09:00",
                        "1997-08-19 09:00",
                        "1997-08-31 09:00",
                    ],
                )],
            ),
            (
                "FREQ=MONTHLY;INTERVAL=2;COUNT=10;BYDAY=1SU,-1SU",
                "1997-09-07 09:00",
                10,
                &[(
                    0,
                    &[
                        "1997-09-07 09:00",
                        "1997-09-28 09:00",
                        "1997-11-02 09:00",
                        "1997-11-30 09:00",
                    ],
                )],
            ),
            (
                "FREQ=MONTHLY;BYMONTHDAY=-3;COUNT=4",
                "1997-09-28 09:00",
                4,
                &[(
                    0,
                    &[
                        "1997-09-28 09:00",
                        "1997-10-29 09:00",
                        "1997-11-28 09:00",
                        "1997-12-29 09:00",
                    ],
                )],
            ),
            (
                "FREQ=MONTHLY;BYMONTHDAY=31;COUNT=4",
                "2026-01-31 09:00",
                4,
                &[(
                    0,
                    &[
                        "2026-01-31 09:00",
                        "2026-03-31 09:00",
                        "2026-05-31 09:00",
                        "2026-07-31 09:00",
                    ],
                )],
            ),
            (
                "FREQ=YEARLY;COUNT=3",
                "2024-02-29 09:00",
                2,
                &[(0, &["2024-02-29 09:00", "2028-02-29 09:00"])],
            ),
            (
                "FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;COUNT=3",
                "1997-09-30 09:00",
                3,
                &[(
                    0,
                    &["1997-09-30 09:00", "1997-10-31 09:00", "1997-11-28 09:00"],
                )],
            ),
            (
                "FREQ=YEARLY;COUNT=10;BYMONTH=6,7",
                "1997-06-10 09:00",
                10,
                &[(
                    0,
                    &[
                        "1997-06-10 09:00",
                        "1997-07-10 09:00",
                        "1998-06-10 09:00",
                        "1998-07-10 09:00",
                    ],
                )],
            ),
            (
                "FREQ=YEARLY;BYWEEKNO=20;BYDAY=MO;COUNT=3",
                "1997-05-12 09:00",
                3,
                &[(
                    0,
                    &["1997-05-12 09:00", "1998-05-11 09:00", "1999-05-17 09:00"],
                )],
            ),
            // Week 1 of 2026 under Monday starts on 2025-12-29.
            (
                "FREQ=YEARLY;BYWEEKNO=1;BYDAY=MO;COUNT=2",
                "2024-12-30 09:00",
                2,
                &[(0, &["2024-12-30 09:00", "2025-12-29 09:00"])],
            ),
            (
                "FREQ=YEARLY;BYYEARDAY=1,100,-1;COUNT=4",
                "2026-01-01 09:00",
                4,
                &[(
                    0,
                    &[
                        "2026-01-01 09:00",
                        "2026-04-10 09:00",
                        "2026-12-31 09:00",
                        "2027-01-01 09:00",
                    ],
                )],
            ),
            (
                "FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13;COUNT=4",
                "1997-09-02 09:00",
                4,
                &[(
                    0,
                    &[
                        "1997-09-02 09:00",
                        "1998-02-13 09:00",
                        "1998-03-13 09:00",
                        "1998-11-13 09:00",
                    ],
                )],
            ),
            (
                "FREQ=DAILY;BYHOUR=9,17;BYMINUTE=0,30;COUNT=5",
                "2026-10-05 09:00",
                5,
                &[(
                    0,
                    &[
                        "2026-10-05 09:00",
                        "2026-10-05 09:30",
                        "2026-10-05 17:00",
                        "2026-10-05 17:30",
                        "2026-10-06 09:00",
                    ],
                )],
            ),
            (
                "FREQ=MINUTELY;INTERVAL=90;BYDAY=MO;COUNT=3",
                "2026-10-05 09:00",
                3,
                &[(
                    0,
                    &["2026-10-05 09:00", "2026-10-05 10:30", "2026-10-05 12:00"],
                )],
            ),
        ];
        for &(rule, dtstart, made, runs) in rows {
            let got = expand(rule, dtstart);
            assert_eq!(got.len(), made, "{rule}: {got:?}");
            for &(from, run) in runs {
                assert_eq!(got[from..from + run.len()], *run, "{rule}");
            }
        }
    }

    #[test]
    fn only_the_window_is_kept_and_count_still_counts_from_dtstart() {
        let rule = Rule::parse("FREQ=DAILY;COUNT=5", None).expect("a rule");
        let got = starts(
            &rule,
            at("2026-10-01 09:00"),
            &|_| true,
            at("2026-10-04 00:00"),
            at("2026-12-01 00:00"),
        );
        assert_eq!(got.starts, [at("2026-10-04 09:00"), at("2026-10-05 09:00")]);
        let none = starts(
            &rule,
            at("2026-10-01 09:00"),
            &|_| true,
            at("2026-10-01 00:00"),
            at("2026-10-01 09:00"),
        );
        assert!(none.starts.is_empty(), "the horizon is exclusive");
    }

    #[test]
    fn an_endless_rule_stops_at_the_horizon() {
        let rule = Rule::parse("FREQ=DAILY", None).expect("a rule");
        let got = starts(
            &rule,
            at("1990-01-01 09:00"),
            &|_| true,
            at("2026-10-01 00:00"),
            at("2026-10-08 00:00"),
        );
        assert_eq!(got.starts.len(), 7);
        assert!(!got.cut);
    }

    proptest::proptest! {
        /// A window is the sum of its halves: what `[from, horizon)` keeps is what `[from, m)`
        /// keeps followed by what `[m, horizon)` keeps, for any split, since each instance is
        /// decided by the rule and `DTSTART` alone and never by where the reader looks.
        #[test]
        fn a_window_is_the_sum_of_its_halves(
            freq in 0usize..4,
            interval in 1u32..4,
            days in proptest::collection::btree_set(0u32..7, 0..4),
            count in proptest::option::of(1u32..40),
            start in 0i64..400,
            split in 0i64..700,
        ) {
            let freq = ["DAILY", "WEEKLY", "MONTHLY", "YEARLY"][freq];
            let byday = if days.is_empty() || freq == "DAILY" {
                String::new()
            } else {
                let names = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"];
                format!(";BYDAY={}", days.iter().map(|d| names[*d as usize]).collect::<Vec<_>>().join(","))
            };
            let count = count.map_or_else(String::new, |n| format!(";COUNT={n}"));
            let rule = Rule::parse(&format!("FREQ={freq};INTERVAL={interval}{byday}{count}"), None).expect("a rule");
            let dtstart = at("2025-01-01 09:30") + Duration::days(start);
            let (from, horizon) = (at("2025-06-01 00:00"), at("2027-06-01 00:00"));
            let middle = from + Duration::days(split);
            let whole = starts(&rule, dtstart, &|_| true, from, horizon).starts;
            let mut halves = starts(&rule, dtstart, &|_| true, from, middle).starts;
            halves.extend(starts(&rule, dtstart, &|_| true, middle, horizon).starts);
            proptest::prop_assert_eq!(&whole, &halves);
            proptest::prop_assert!(whole.windows(2).all(|pair| pair[0] < pair[1]), "ascending, no twins");
        }
    }
}

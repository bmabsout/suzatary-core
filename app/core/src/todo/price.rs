//! What a todo is worth, as every writer authors it and a proposed act stores it.

use chrono::NaiveDate;
use prodrome_core::fpl;
use prodrome_core::literal::{ProdromeError, Value};
use prodrome_core::term::{Term, TermF};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::literal::{Fields, Literal, Signature, arm, no_arm, record};
use crate::refusal::Refusal;
use crate::todo::{Cycle, Deadline};
use crate::zone::Moment;

/// One price, since a todo carrying two has no defined value. Percentages are whole; low is
/// urgent. The serde form is a verb's argument, and the inbox's acts' before they were literals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "how", rename_all = "lowercase")]
pub enum Price {
    /// A fulfillment that does not move.
    Flat {
        /// The number.
        percent: u32,
    },
    /// A fulfillment that decays toward a date.
    Deadline(Deadline),
    /// A chore whose cycle restarts at every pass (`Tended`); before the first pass it is the
    /// price it replaces, or for a new todo the cycle from its creation.
    Recur(Cycle),
    /// The cycle repeated on the calendar from the instant it is authored, whatever is done.
    Periodic(Cycle),
}

impl Price {
    /// The term this price is, for `todo`, authored at `now` over `replaced`, the todo's own term
    /// before it (`None` for a new todo).
    ///
    /// A `Recur` over a todo already recurring waits on that one's pending price rather than
    /// nesting it: with no pass yet the two read the same, and after one neither reads it.
    ///
    /// # Errors
    /// A percentage over 100, a cycle or lead-up out of range, or a term the evaluator refuses.
    pub fn term(&self, todo: &str, now: Moment, replaced: Option<&Term>) -> Result<Term, Refusal> {
        let anchor = fpl::instant_of(now.at);
        match self {
            Price::Flat { percent } => fpl::mk_flat(ratio(*percent, "priority")?).map_err(refused),
            Price::Deadline(due) => due.term(now.zone),
            Price::Recur(cycle) => {
                let body = cycle.ramp(anchor)?;
                let pending = match replaced {
                    None => body.clone(),
                    Some(term) => match term.out() {
                        TermF::Recur {
                            todo: over,
                            pending,
                            ..
                        } if over == todo => pending.clone(),
                        _ => term.clone(),
                    },
                };
                fpl::mk_recur(todo.to_owned(), anchor, body, pending).map_err(refused)
            }
            Price::Periodic(cycle) => {
                fpl::mk_periodic(cycle.span(), anchor, cycle.ramp(anchor)?).map_err(refused)
            }
        }
    }
}

impl Price {
    /// The price `term` is, when it is a term [`Price::term`] writes, read in `zone`: a writer's
    /// term carried into an act again. Law: whatever this reads writes the same term back, at
    /// the term's own anchor for a cycle (`a_read_price_writes_its_term_back`).
    #[must_use]
    pub fn read(term: &Term, todo: &str, zone: crate::zone::Zone) -> Option<Price> {
        let price = match term.out() {
            TermF::Flat { value } => Price::Flat {
                percent: percent(*value)?,
            },
            TermF::Decay {
                start,
                end,
                end_date,
                lead_up,
                start_date,
            } => Price::Deadline(Deadline {
                end_date: day_of(zone.day(*end_date), |day| zone.end(day) == *end_date)?,
                start_percent: percent(*start)?,
                end_percent: percent(*end)?,
                lead_up: (*lead_up != chrono::Duration::weeks(1)).then_some(*lead_up),
                start_date: match start_date {
                    None => None,
                    Some(at) => Some(day_of(zone.day(*at), |day| zone.start(day) == *at)?),
                },
            }),
            TermF::Recur { term: body, .. } => Price::Recur(ramp_cycle(body)?),
            TermF::Periodic { term: body, .. } => Price::Periodic(ramp_cycle(body)?),
            _ => return None,
        };
        let anchor = match term.out() {
            TermF::Recur { anchor, .. } | TermF::Periodic { anchor, .. } => *anchor,
            _ => chrono::NaiveDateTime::default(),
        };
        let now = Moment {
            at: fpl::datetime_of(anchor).ok()?,
            zone,
        };
        (price.term(todo, now, Some(term)).ok()? == *term).then_some(price)
    }
}

/// A ratio as the whole percentage it was written as.
fn percent(value: f64) -> Option<u32> {
    let scaled = (value * 100.0).round();
    ((scaled - value * 100.0).abs() < 1e-9 && (0.0..=100.0).contains(&scaled))
        .then(|| format!("{scaled:.0}").parse().ok())
        .flatten()
}

/// The day near `near` that `is` holds for: a day's end may fall on the next day's first instant.
fn day_of(near: NaiveDate, is: impl Fn(NaiveDate) -> bool) -> Option<NaiveDate> {
    [Some(near), near.pred_opt()]
        .into_iter()
        .flatten()
        .find(|day| is(*day))
}

/// The cycle a two-point ramp is.
fn ramp_cycle(ramp: &Term) -> Option<Cycle> {
    let TermF::Curve { points } = ramp.out() else {
        return None;
    };
    let [from, to] = points.as_slice() else {
        return None;
    };
    let span = to.at - from.at;
    let days = u32::try_from(span.num_days()).ok()?;
    if span != chrono::Duration::days(i64::from(days)) {
        return None;
    }
    Some(Cycle {
        days,
        from_percent: percent(from.value)?,
        to_percent: percent(to.value)?,
    })
}

fn refused(failed: impl std::fmt::Display) -> Refusal {
    Refusal::bad_request(failed.to_string())
}

/// A whole percentage as the ratio the evaluator takes.
///
/// # Errors
/// Over 100, naming `field`.
pub fn ratio(percent: u32, field: &str) -> Result<f64, Refusal> {
    if percent > 100 {
        return Err(Refusal::bad_request(format!(
            "{field} must be a whole-number percentage like '65%', got {percent}%. \
             It is a FULFILLMENT, so LOW means urgent."
        )));
    }
    Ok(f64::from(percent) / 100.0)
}

/// `FlatPrice(percent)`; the term vocabulary's `Flat` is a term, not a price.
pub const FLAT_PRICE: Signature = ("FlatPrice", &["percent"]);
/// `DeadlinePrice(end_date, start_percent, end_percent, lead_up_days, start_date)`.
pub const DEADLINE_PRICE: Signature = (
    "DeadlinePrice",
    &[
        "end_date",
        "start_percent",
        "end_percent",
        "lead_up_days",
        "start_date",
    ],
);
/// `RecurPrice(days, from_percent, to_percent)`.
pub const RECUR_PRICE: Signature = ("RecurPrice", &["days", "from_percent", "to_percent"]);
/// `PeriodicPrice(days, from_percent, to_percent)`.
pub const PERIODIC_PRICE: Signature = ("PeriodicPrice", &["days", "from_percent", "to_percent"]);

fn cycle(signature: Signature, cycle: &Cycle) -> Value {
    record(
        signature,
        vec![
            cycle.days.to_value(),
            cycle.from_percent.to_value(),
            cycle.to_percent.to_value(),
        ],
    )
}

fn cycle_of(value: &Value, signature: Signature) -> Result<Cycle, ProdromeError> {
    let fields = Fields::of(value, signature)?;
    Ok(Cycle {
        days: fields.get("days")?,
        from_percent: fields.get("from_percent")?,
        to_percent: fields.get("to_percent")?,
    })
}

impl Literal for Price {
    fn to_value(&self) -> Value {
        match self {
            Price::Flat { percent } => record(FLAT_PRICE, vec![percent.to_value()]),
            Price::Deadline(due) => record(
                DEADLINE_PRICE,
                vec![
                    due.end_date.to_value(),
                    due.start_percent.to_value(),
                    due.end_percent.to_value(),
                    due.lead_up.to_value(),
                    due.start_date.to_value(),
                ],
            ),
            Price::Recur(recur) => cycle(RECUR_PRICE, recur),
            Price::Periodic(periodic) => cycle(PERIODIC_PRICE, periodic),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        Ok(match arm(value)? {
            "FlatPrice" => Price::Flat {
                percent: Fields::of(value, FLAT_PRICE)?.get("percent")?,
            },
            "DeadlinePrice" => {
                let fields = Fields::of(value, DEADLINE_PRICE)?;
                Price::Deadline(Deadline {
                    end_date: fields.get("end_date")?,
                    start_percent: fields.get("start_percent")?,
                    end_percent: fields.get("end_percent")?,
                    lead_up: fields.get("lead_up_days")?,
                    start_date: fields.get("start_date")?,
                })
            }
            "RecurPrice" => Price::Recur(cycle_of(value, RECUR_PRICE)?),
            "PeriodicPrice" => Price::Periodic(cycle_of(value, PERIODIC_PRICE)?),
            other => return Err(no_arm("a price", other)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(text: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a day")
    }

    /// The shapes as the inbox stores them.
    #[test]
    fn a_price_prints_as_a_stored_act_spells_it() {
        let printed = |price: &Price| serde_json::to_string(price).expect("prints");
        assert_eq!(
            printed(&Price::Flat { percent: 40 }),
            r#"{"how":"flat","percent":40}"#
        );
        assert_eq!(
            printed(&Price::Deadline(Deadline {
                end_date: day("2026-10-20"),
                start_percent: 70,
                end_percent: 10,
                lead_up: Some(chrono::Duration::weeks(3)),
                start_date: None,
            })),
            r#"{"how":"deadline","end_date":"2026-10-20","start_percent":70,"end_percent":10,"lead_up_weeks":3,"start_date":null}"#
        );
        assert_eq!(
            printed(&Price::Recur(Cycle {
                days: 60,
                from_percent: 100,
                to_percent: 30
            })),
            r#"{"how":"recur","days":60,"from_percent":100,"to_percent":30}"#
        );
    }

    #[test]
    fn a_lead_up_in_days_reads_back_as_days() {
        let due = Price::Deadline(Deadline {
            end_date: day("2026-10-20"),
            start_percent: 60,
            end_percent: 5,
            lead_up: Some(chrono::Duration::days(10)),
            start_date: None,
        });
        let printed = serde_json::to_string(&due).expect("prints");
        assert!(
            printed.contains(r#""lead_up_weeks":null,"lead_up_days":10"#),
            "{printed}"
        );
        assert_eq!(serde_json::from_str::<Price>(&printed).expect("reads"), due);
        assert!(
            serde_json::from_str::<Price>(
                r#"{"how":"deadline","end_date":"2026-10-20","start_percent":60,"end_percent":5,"lead_up_weeks":1,"lead_up_days":3,"start_date":null}"#
            )
            .is_err()
        );
    }

    fn prices() -> impl proptest::strategy::Strategy<Value = Price> {
        use proptest::prelude::*;
        let cycle = (1u32..=3650, 0u32..=100, 0u32..=100).prop_map(|(days, from, to)| Cycle {
            days,
            from_percent: from,
            to_percent: to,
        });
        let day =
            (0i64..2000).prop_map(|offset| day("2025-01-01") + chrono::Duration::days(offset));
        prop_oneof![
            (0u32..=100).prop_map(|percent| Price::Flat { percent }),
            (
                day.clone(),
                0u32..=98,
                0u32..=100,
                proptest::option::of(1i64..=364),
                proptest::option::of(day)
            )
                .prop_map(
                    |(end_date, start_percent, end_percent, lead, start_date)| {
                        Price::Deadline(Deadline {
                            end_date,
                            start_percent,
                            end_percent,
                            lead_up: lead.map(chrono::Duration::days),
                            start_date,
                        })
                    }
                ),
            cycle.clone().prop_map(Price::Recur),
            cycle.prop_map(Price::Periodic),
        ]
    }

    proptest::proptest! {
        /// Law: the price read from a term a price writes writes that same term, in any zone and
        /// at any instant, so carrying a stored term into an act moves no number.
        #[test]
        fn a_read_price_writes_its_term_back(
            price in prices(),
            minutes in 0i64..1_000_000,
            named in proptest::bool::ANY,
        ) {
            let zone = if named {
                crate::zone::Zone::named("America/New_York").expect("a zone")
            } else {
                crate::zone::Zone::UTC
            };
            let at = fpl::datetime_of(
                day("2025-01-01").and_hms_opt(0, 0, 0).expect("midnight")
                    + chrono::Duration::minutes(minutes),
            )
            .expect("an instant");
            let now = Moment { at, zone };
            let Ok(term) = price.term("chore", now, None) else {
                return Ok(());
            };
            let read = Price::read(&term, "chore", zone);
            proptest::prop_assert!(read.is_some(), "{}", fpl::print_term(&term));
            let written = read.and_then(|read| read.term("chore", now, None).ok());
            proptest::prop_assert_eq!(written, Some(term));
        }
    }
}

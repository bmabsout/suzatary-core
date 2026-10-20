//! What this box has spent of its Claude allowance: a fold over parsed
//! records, deciding and reading nothing. Quota is the constraint up to the
//! allowance and credits bill past it, so the two are never blurred and no
//! price is invented. Where the allowance is unknown the report gives a
//! measured floor, never a percentage of a guess.

use std::collections::BTreeMap;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One request, as the transcripts record it and this module reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// When it was made.
    pub at: DateTime<Utc>,
    /// The model's own id, verbatim.
    pub model: String,
    /// Who made it, as far as the reader can honestly tell.
    pub who: Who,
    /// Its tokens.
    pub tokens: Tokens,
}

/// Who spent it, as far as a reader that takes no text can tell: triage and
/// the secretary share a transcript directory, so they are one arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Who {
    /// The interactive assistant session.
    Session,
    /// The triage timer and the secretary, together.
    Automated,
    /// A transcript directory the reader could not place.
    Other,
}

/// A request's four token counts, kept apart because each is priced differently.
/// `number` in TypeScript, not ts-rs's `bigint`: `JSON.parse` never makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, TS)]
#[ts(rename = "UsageTokens")]
pub struct Tokens {
    /// Input the model read for the first time.
    #[ts(type = "number")]
    pub fresh: u64,
    /// Input written into the prompt cache — paid at a premium.
    #[ts(type = "number")]
    pub cache_write: u64,
    /// Input served from the prompt cache — paid at a fraction.
    #[ts(type = "number")]
    pub cache_read: u64,
    /// Output.
    #[ts(type = "number")]
    pub output: u64,
}

impl Tokens {
    /// Everything the request read, cached or not.
    #[must_use]
    pub fn input(self) -> u64 {
        self.fresh + self.cache_write + self.cache_read
    }

    /// Everything, both directions.
    #[must_use]
    pub fn total(self) -> u64 {
        self.input() + self.output
    }

    /// How much of the input was served from cache, in `[0, 1]`; `None` when
    /// there was no input at all, because a rate over nothing is not zero.
    #[must_use]
    pub fn cache_hit(self) -> Option<f64> {
        let input = self.input();
        if input == 0 {
            None
        } else {
            // Both fit a mantissa many times over at any plausible scale.
            #[allow(clippy::cast_precision_loss)]
            Some(self.cache_read as f64 / input as f64)
        }
    }

    fn add(self, other: Tokens) -> Tokens {
        Tokens {
            fresh: self.fresh + other.fresh,
            cache_write: self.cache_write + other.cache_write,
            cache_read: self.cache_read + other.cache_read,
            output: self.output + other.output,
        }
    }
}

/// The two windows the limits are measured in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Window {
    /// The rolling five-hour block.
    FiveHour,
    /// The rolling seven days.
    SevenDay,
}

impl Window {
    /// How long it is.
    #[must_use]
    pub fn length(self) -> Duration {
        match self {
            Window::FiveHour => Duration::hours(5),
            Window::SevenDay => Duration::days(7),
        }
    }

    /// The window the limits call `name`, by the one spelling serde derives; `None` for a
    /// limit this box does not count.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(name))
            .ok()
    }
}

/// A request the limit refused, with what the limit said about itself; the
/// one place the transcripts speak about the allowance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "UsageRefusal")]
pub struct Refused {
    /// When.
    #[serde(with = "stamp")]
    #[ts(type = "string")]
    pub at: DateTime<Utc>,
    /// Which window refused; the report already files it under that window.
    #[serde(skip)]
    #[ts(skip)]
    pub window: Window,
    /// When that window resets, if the limit said.
    #[serde(with = "maybe_stamp")]
    #[ts(type = "string | null")]
    pub resets_at: Option<DateTime<Utc>>,
    /// The credit situation at that moment.
    #[serde(flatten)]
    #[ts(flatten)]
    pub overage: Overage,
}

/// Whether crossing the allowance bills, from the refusal's own metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
pub struct Overage {
    /// Past the allowance, requests bill as credits rather than refuse.
    #[serde(rename = "overage_enabled")]
    #[ts(rename = "overage_enabled")]
    pub enabled: bool,
    /// This request was itself on credits.
    #[serde(rename = "overage_using")]
    #[ts(rename = "overage_using")]
    pub using: bool,
    /// The limit's own reason when it is not enabled, verbatim, or empty.
    #[serde(rename = "overage_reason")]
    #[ts(rename = "overage_reason")]
    pub disabled_reason: String,
}

/// One reading of a window's allowance. The source answers about hourly, so
/// the page says how old it is.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "UsageReported")]
pub struct Reported {
    /// How much of it is used, in `[0, 1]`.
    pub utilization: f64,
    /// When it resets, if reported.
    #[serde(with = "maybe_stamp")]
    #[ts(type = "string | null")]
    pub resets_at: Option<DateTime<Utc>>,
    /// When this reading was taken.
    #[serde(with = "stamp")]
    #[ts(type = "string")]
    pub as_of: DateTime<Utc>,
}

/// What this box knows about one window's allowance. Only
/// [`Allowance::Reported`] carries a fraction, so a page cannot print a
/// percentage of the measured floor as if it were the allowance.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(rename = "UsageAllowance")]
pub enum Allowance {
    /// The endpoint answered.
    Reported(Reported),
    /// The endpoint answers 429; the last good reading rides along.
    RateLimited {
        /// The first 429 after a success, or the first read.
        #[serde(with = "stamp")]
        #[ts(type = "string")]
        since: DateTime<Utc>,
        /// The endpoint's own `retry-after`, in seconds.
        #[ts(type = "number")]
        retry_after_seconds: u64,
        /// The last successful reading, if this box ever got one.
        last_reported: Option<Reported>,
    },
    /// The endpoint refuses this box's token, which waiting will not change: a 403
    /// `oauth_scope_insufficient` says the setup-token lacks `user:profile`.
    Refused {
        /// The last refusal.
        #[serde(with = "stamp")]
        #[ts(type = "string")]
        at: DateTime<Utc>,
        /// Its HTTP status.
        status: u16,
        /// Its error's own words, verbatim and capped: the endpoint's, never mail's.
        words: String,
    },
    /// Never read: no attempt has answered either way yet.
    Unknown,
}

/// Which project directories this report read and which it found but did
/// not: the allowance is the whole account's, the totals only this box's.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, TS)]
#[ts(rename = "UsageScope")]
pub struct Scope {
    /// Directory slugs that contributed a request or a refusal.
    pub counted: Vec<String>,
    /// Directory slugs found but older than the reader's lookback.
    pub not_counted: Vec<String>,
}

/// One model's share of a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "UsageModelRow")]
pub struct ModelShare {
    /// The model id, verbatim.
    pub model: String,
    /// Its tokens.
    pub tokens: Tokens,
}

/// One author's share of a window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(rename = "UsageWhoRow")]
pub struct WhoShare {
    /// Who.
    #[ts(type = "string")]
    pub who: Who,
    /// Their tokens.
    pub tokens: Tokens,
}

/// One window's report.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "UsageWindow")]
pub struct WindowReport {
    /// Which window this is: `five_hour` or `seven_day`, the limits' own names.
    #[ts(type = "string")]
    pub window: Window,
    /// The window's start, `now - length`.
    #[serde(with = "stamp")]
    #[ts(type = "string")]
    pub since: DateTime<Utc>,
    /// Requests inside it.
    pub requests: usize,
    /// Their tokens, summed.
    pub tokens: Tokens,
    /// By model, largest total first.
    pub by_model: Vec<ModelShare>,
    /// By who, in `Who`'s order.
    pub by_who: Vec<WhoShare>,
    /// Reads over all input, `[0, 1]`; `None` over no input.
    pub cache_hit: Option<f64>,
    /// The last refusal this window produced, ever, not only inside it.
    pub last_refusal: Option<Refused>,
    /// What credits are charged on: spent after the last refusal while overage
    /// was enabled. `None`, not zero, without such a refusal.
    pub on_credits: Option<Tokens>,
    /// Spent in one window-length before the last refusal: a floor of the
    /// limit, never divided into.
    pub most_seen_before_refusal: Option<Tokens>,
    /// What this box knows about the allowance.
    pub allowance: Allowance,
    /// [`Who::Other`]'s share, named beside the allowance bar; zero, not
    /// absent, when nothing else spent.
    pub elsewhere: Tokens,
}

/// The whole report: `GET /api/usage`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[ts(rename = "UsagePayload")]
pub struct Report {
    /// The instant it was folded at.
    #[serde(with = "stamp")]
    #[ts(type = "string")]
    pub at: DateTime<Utc>,
    /// The five-hour window.
    pub five_hour: WindowReport,
    /// The seven-day window.
    pub seven_day: WindowReport,
    /// Everything the reader saw, summed.
    pub seen: Tokens,
    /// How many requests the reader saw.
    pub seen_requests: usize,
    /// The reader's horizon, so "seen" is never read as all time.
    #[serde(with = "maybe_stamp")]
    #[ts(type = "string | null")]
    pub seen_since: Option<DateTime<Utc>>,
    /// Which directories it read.
    pub scope: Scope,
}

/// Fold the records into the report. Records come in any order; of two
/// allowances for one window the later wins, and none reads
/// [`Allowance::Unknown`].
#[must_use]
pub fn report(
    at: DateTime<Utc>,
    requests: &[Request],
    refusals: &[Refused],
    allowances: &[(Window, Allowance)],
    horizon: Option<DateTime<Utc>>,
    scope: Scope,
) -> Report {
    let seen = requests
        .iter()
        .fold(Tokens::default(), |sum, request| sum.add(request.tokens));
    Report {
        at,
        five_hour: window(Window::FiveHour, at, requests, refusals, allowances),
        seven_day: window(Window::SevenDay, at, requests, refusals, allowances),
        seen,
        seen_requests: requests.len(),
        seen_since: horizon,
        scope,
    }
}

fn window(
    which: Window,
    at: DateTime<Utc>,
    requests: &[Request],
    refusals: &[Refused],
    allowances: &[(Window, Allowance)],
) -> WindowReport {
    let since = at - which.length();
    let inside: Vec<&Request> = requests
        .iter()
        .filter(|request| request.at > since && request.at <= at)
        .collect();

    let tokens = inside
        .iter()
        .fold(Tokens::default(), |sum, request| sum.add(request.tokens));

    let mut by_model: BTreeMap<&str, Tokens> = BTreeMap::new();
    let mut by_who: BTreeMap<Who, Tokens> = BTreeMap::new();
    for request in &inside {
        let model = by_model.entry(request.model.as_str()).or_default();
        *model = model.add(request.tokens);
        let who = by_who.entry(request.who).or_default();
        *who = who.add(request.tokens);
    }
    let mut by_model: Vec<ModelShare> = by_model
        .into_iter()
        .map(|(model, tokens)| ModelShare {
            model: model.to_owned(),
            tokens,
        })
        .collect();
    // Largest first, then by name so equal totals are stable.
    by_model.sort_by(|left, right| {
        right
            .tokens
            .total()
            .cmp(&left.tokens.total())
            .then_with(|| left.model.cmp(&right.model))
    });
    let elsewhere = by_who.get(&Who::Other).copied().unwrap_or_default();
    let by_who: Vec<WhoShare> = by_who
        .into_iter()
        .map(|(who, tokens)| WhoShare { who, tokens })
        .collect();

    let last_refusal = refusals
        .iter()
        .filter(|refused| refused.window == which)
        .max_by_key(|refused| refused.at)
        .cloned();

    let on_credits = last_refusal.as_ref().and_then(|refused| {
        if !refused.overage.enabled {
            return None;
        }
        Some(
            requests
                .iter()
                .filter(|request| request.at > refused.at && request.at <= at)
                .fold(Tokens::default(), |sum, request| sum.add(request.tokens)),
        )
    });

    let most_seen_before_refusal = last_refusal.as_ref().map(|refused| {
        let from = refused.at - which.length();
        requests
            .iter()
            .filter(|request| request.at > from && request.at <= refused.at)
            .fold(Tokens::default(), |sum, request| sum.add(request.tokens))
    });

    let allowance = allowances
        .iter()
        .rev()
        .find(|(window, _)| *window == which)
        .map_or(Allowance::Unknown, |(_, allowance)| allowance.clone());

    WindowReport {
        window: which,
        since,
        requests: inside.len(),
        tokens,
        by_model,
        by_who,
        cache_hit: tokens.cache_hit(),
        last_refusal,
        on_credits,
        most_seen_before_refusal,
        allowance,
        elsewhere,
    }
}

/// RFC 3339 to the second in UTC with a `Z`.
mod stamp {
    use chrono::{DateTime, SecondsFormat, Utc};
    use serde::Serializer;

    pub fn serialize<S: Serializer>(at: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&at.to_rfc3339_opts(SecondsFormat::Secs, true))
    }
}

mod maybe_stamp {
    use chrono::{DateTime, Utc};
    use serde::Serializer;

    #[expect(clippy::ref_option, reason = "serde's `with` signature")]
    pub fn serialize<S: Serializer>(
        at: &Option<DateTime<Utc>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match at {
            Some(at) => super::stamp::serialize(at, serializer),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window's name reads back as the window it printed, and a limit the box does not
    /// count reads as none.
    #[test]
    fn a_window_reads_back_by_the_name_it_prints() {
        for window in [Window::FiveHour, Window::SevenDay] {
            let printed = serde_json::to_value(window).expect("a name");
            assert_eq!(
                Window::named(printed.as_str().expect("a string")),
                Some(window)
            );
        }
        assert_eq!(Window::named("seven_day_opus"), None);
    }

    fn at(hours_ago: i64) -> DateTime<Utc> {
        now() - Duration::hours(hours_ago)
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .expect("a real instant")
            .with_timezone(&Utc)
    }

    fn request(hours_ago: i64, model: &str, who: Who, tokens: Tokens) -> Request {
        Request {
            at: at(hours_ago),
            model: model.to_owned(),
            who,
            tokens,
        }
    }

    fn some(fresh: u64, cache_write: u64, cache_read: u64, output: u64) -> Tokens {
        Tokens {
            fresh,
            cache_write,
            cache_read,
            output,
        }
    }

    /// The windows are rolling and half-open, `since < at <= now`: a request exactly five hours
    /// old is out of the block and in the week.
    #[test]
    fn a_window_holds_what_fell_inside_it_and_nothing_older() {
        let requests = [
            request(1, "opus", Who::Session, some(1, 0, 0, 1)),
            request(5, "opus", Who::Session, some(10, 0, 0, 10)),
            request(24 * 6, "sonnet", Who::Automated, some(100, 0, 0, 100)),
            request(24 * 8, "sonnet", Who::Automated, some(1000, 0, 0, 1000)),
        ];
        let report = report(now(), &requests, &[], &[], None, Scope::default());
        assert_eq!(report.five_hour.requests, 1);
        assert_eq!(report.five_hour.tokens.total(), 2);
        assert_eq!(report.seven_day.requests, 3, "the 8-day-old one is out");
        assert_eq!(report.seven_day.tokens.total(), 222);
        assert_eq!(report.seen_requests, 4);
        assert_eq!(report.seen.total(), 2222);
        assert_eq!(report.seen_since, None, "everything was read");
    }

    /// By model is largest first, by who is in `Who`'s order, and a model that spent nothing in
    /// the window is not a row.
    #[test]
    fn the_splits_are_ordered_and_only_name_what_spent() {
        let requests = [
            request(1, "sonnet", Who::Automated, some(5, 0, 0, 5)),
            request(2, "opus", Who::Session, some(50, 0, 0, 50)),
            request(3, "opus", Who::Automated, some(1, 0, 0, 1)),
            request(24 * 2, "haiku", Who::Other, some(9, 0, 0, 9)),
        ];
        let report = report(now(), &requests, &[], &[], None, Scope::default());
        let models: Vec<&str> = report
            .five_hour
            .by_model
            .iter()
            .map(|share| share.model.as_str())
            .collect();
        assert_eq!(models, ["opus", "sonnet"], "haiku is outside the block");
        assert_eq!(report.five_hour.by_model[0].tokens.total(), 102);
        let who: Vec<Who> = report
            .five_hour
            .by_who
            .iter()
            .map(|share| share.who)
            .collect();
        assert_eq!(who, [Who::Session, Who::Automated]);
    }

    /// Cache hit is reads over all input, and undefined rather than zero over nothing.
    #[test]
    fn cache_hit_is_a_rate_and_is_undefined_over_nothing() {
        assert_eq!(some(10, 10, 80, 0).cache_hit(), Some(0.8));
        assert_eq!(some(0, 0, 0, 5).cache_hit(), None);
        let report = report(now(), &[], &[], &[], None, Scope::default());
        assert_eq!(report.five_hour.tokens.cache_hit(), None);
    }

    /// The last refusal is per window and ever, not only inside the window.
    #[test]
    fn the_last_refusal_is_the_latest_of_its_window_however_old() {
        let refusals = [
            Refused {
                at: at(30),
                window: Window::SevenDay,
                resets_at: Some(at(-10)),
                overage: Overage {
                    enabled: false,
                    using: false,
                    disabled_reason: "org_level_disabled_until".to_owned(),
                },
            },
            Refused {
                at: at(60),
                window: Window::SevenDay,
                resets_at: None,
                overage: Overage {
                    enabled: false,
                    using: false,
                    disabled_reason: String::new(),
                },
            },
            Refused {
                at: at(2),
                window: Window::FiveHour,
                resets_at: None,
                overage: Overage {
                    enabled: false,
                    using: false,
                    disabled_reason: String::new(),
                },
            },
        ];
        let report = report(now(), &[], &refusals, &[], None, Scope::default());
        assert_eq!(
            report.seven_day.last_refusal.as_ref().map(|r| r.at),
            Some(at(30)),
            "the later of the two weekly refusals, though outside the block"
        );
        assert_eq!(
            report.five_hour.last_refusal.as_ref().map(|r| r.at),
            Some(at(2))
        );
    }

    /// Credits are what was spent after the refusal while overage was on, and `None`, not zero,
    /// when overage was off.
    #[test]
    fn on_credits_is_spend_after_the_refusal_only_when_overage_was_enabled() {
        let requests = [
            request(10, "opus", Who::Session, some(100, 0, 0, 0)),
            request(3, "opus", Who::Session, some(7, 0, 0, 0)),
            request(1, "opus", Who::Session, some(5, 0, 0, 0)),
        ];
        let refused = |enabled: bool| Refused {
            at: at(4),
            window: Window::SevenDay,
            resets_at: None,
            overage: Overage {
                enabled,
                using: enabled,
                disabled_reason: String::new(),
            },
        };
        let on = report(
            now(),
            &requests,
            &[refused(true)],
            &[],
            None,
            Scope::default(),
        );
        assert_eq!(on.seven_day.on_credits.map(Tokens::total), Some(12));
        let off = report(
            now(),
            &requests,
            &[refused(false)],
            &[],
            None,
            Scope::default(),
        );
        assert_eq!(off.seven_day.on_credits, None);
    }

    /// The ceiling is what was spent in one window-length before the refusal: a floor of the
    /// real limit, never a fraction.
    #[test]
    fn most_seen_before_refusal_is_the_window_before_it() {
        let requests = [
            request(24 * 9, "opus", Who::Session, some(1000, 0, 0, 0)), // before the window
            request(24 * 6, "opus", Who::Session, some(100, 0, 0, 0)),  // inside it
            request(24 * 2, "opus", Who::Session, some(10, 0, 0, 0)),   // inside it
            request(1, "opus", Who::Session, some(1, 0, 0, 0)),         // after the refusal
        ];
        let refusals = [Refused {
            at: at(24),
            window: Window::SevenDay,
            resets_at: None,
            overage: Overage {
                enabled: false,
                using: false,
                disabled_reason: String::new(),
            },
        }];
        let report = report(now(), &requests, &refusals, &[], None, Scope::default());
        assert_eq!(
            report.seven_day.most_seen_before_refusal.map(Tokens::total),
            Some(110)
        );
    }

    /// A window with no allowance entry reads `Unknown`, never an empty reading.
    #[test]
    fn a_window_with_no_allowance_entry_is_unknown() {
        let report = report(now(), &[], &[], &[], None, Scope::default());
        assert_eq!(report.five_hour.allowance, Allowance::Unknown);
        assert_eq!(report.seven_day.allowance, Allowance::Unknown);
    }

    /// The allowance is per window and the latest entry wins.
    #[test]
    fn the_reported_allowance_is_the_last_entry_for_its_window() {
        let allowances = [
            (
                Window::FiveHour,
                Allowance::Reported(Reported {
                    utilization: 0.2,
                    resets_at: None,
                    as_of: at(2),
                }),
            ),
            (
                Window::FiveHour,
                Allowance::Reported(Reported {
                    utilization: 0.4,
                    resets_at: Some(at(-1)),
                    as_of: at(1),
                }),
            ),
        ];
        let report = report(now(), &[], &[], &allowances, None, Scope::default());
        assert_eq!(
            report.five_hour.allowance,
            Allowance::Reported(Reported {
                utilization: 0.4,
                resets_at: Some(at(-1)),
                as_of: at(1),
            })
        );
        assert_eq!(report.seven_day.allowance, Allowance::Unknown);
    }

    /// Rate-limited carries its own last reading, never `measured_ceiling`'s.
    #[test]
    fn rate_limited_carries_its_own_last_reading_and_not_the_measured_one() {
        let last = Reported {
            utilization: 0.42,
            resets_at: Some(at(-2)),
            as_of: at(3),
        };
        let allowances = [(
            Window::SevenDay,
            Allowance::RateLimited {
                since: at(1),
                retry_after_seconds: 3472,
                last_reported: Some(last.clone()),
            },
        )];
        let report = report(now(), &[], &[], &allowances, None, Scope::default());
        assert_eq!(
            report.seven_day.allowance,
            Allowance::RateLimited {
                since: at(1),
                retry_after_seconds: 3472,
                last_reported: Some(last),
            }
        );
    }

    /// Rate-limited with no last reading is its own shape, never a zero percentage.
    #[test]
    fn rate_limited_with_no_prior_success_carries_none() {
        let allowances = [(
            Window::FiveHour,
            Allowance::RateLimited {
                since: at(0),
                retry_after_seconds: 3600,
                last_reported: None,
            },
        )];
        let report = report(now(), &[], &[], &allowances, None, Scope::default());
        assert_eq!(
            report.five_hour.allowance,
            Allowance::RateLimited {
                since: at(0),
                retry_after_seconds: 3600,
                last_reported: None,
            }
        );
    }

    /// `elsewhere` is `Who::Other`'s share of the window, and is not subtracted from `tokens`,
    /// which already sums every `Who`.
    #[test]
    fn elsewhere_is_the_other_directories_own_share_and_nothing_is_double_counted() {
        let requests = [
            request(1, "opus", Who::Session, some(10, 0, 0, 0)),
            request(1, "sonnet", Who::Automated, some(20, 0, 0, 0)),
            request(1, "haiku", Who::Other, some(30, 0, 0, 0)),
        ];
        let report = report(now(), &requests, &[], &[], None, Scope::default());
        assert_eq!(report.five_hour.elsewhere.total(), 30);
        assert_eq!(
            report.five_hour.tokens.total(),
            60,
            "the window's own total already includes every Who"
        );
    }

    /// A window with no `Other` request reads `elsewhere` as zero, not absent.
    #[test]
    fn elsewhere_is_zero_and_not_absent_when_nothing_else_was_read() {
        let requests = [request(1, "opus", Who::Session, some(10, 0, 0, 0))];
        let report = report(now(), &requests, &[], &[], None, Scope::default());
        assert_eq!(report.five_hour.elsewhere, Tokens::default());
    }

    /// The scope passes through unchanged: the reader decided which directories were read.
    #[test]
    fn the_scope_passes_through_the_report_unchanged() {
        let scope = Scope {
            counted: vec!["-home-me".to_owned(), "-home-me-repo".to_owned()],
            not_counted: vec!["-tmp".to_owned()],
        };
        let report = report(now(), &[], &[], &[], None, scope.clone());
        assert_eq!(report.scope, scope);
    }

    /// The report is `/api/usage`'s body: instants to the second with a `Z`,
    /// shares as rows, a refusal's overage flattened beside it.
    #[test]
    fn the_report_serialises_as_the_usage_payload() {
        let requests = [
            request(1, "opus", Who::Session, some(10, 0, 30, 5)),
            request(30, "opus", Who::Automated, some(1, 0, 0, 1)),
        ];
        let refusals = [Refused {
            at: at(2) + Duration::milliseconds(250),
            window: Window::FiveHour,
            resets_at: Some(at(-3)),
            overage: Overage {
                enabled: true,
                using: false,
                disabled_reason: String::new(),
            },
        }];
        let reported = Reported {
            utilization: 0.5,
            resets_at: None,
            as_of: at(1),
        };
        let allowances = [
            (Window::FiveHour, Allowance::Reported(reported.clone())),
            (
                Window::SevenDay,
                Allowance::RateLimited {
                    since: at(4),
                    retry_after_seconds: 60,
                    last_reported: Some(reported),
                },
            ),
        ];
        let report = report(
            now(),
            &requests,
            &refusals,
            &allowances,
            Some(at(48)),
            Scope::default(),
        );
        let json = serde_json::to_value(&report).expect("serialises");
        let tokens = |fresh: u64, cache_read: u64, output: u64| serde_json::json!({"fresh": fresh, "cache_write": 0, "cache_read": cache_read, "output": output});
        let refusal = serde_json::json!({
            "at": "2026-09-12T10:00:00Z", "resets_at": "2026-09-12T15:00:00Z",
            "overage_enabled": true, "overage_using": false, "overage_reason": "",
        });
        let reported = serde_json::json!({"utilization": 0.5, "resets_at": null, "as_of": "2026-09-12T11:00:00Z"});
        assert_eq!(
            json,
            serde_json::json!({
                "at": "2026-09-12T12:00:00Z",
                "five_hour": {
                    "window": "five_hour", "since": "2026-09-12T07:00:00Z", "requests": 1,
                    "tokens": tokens(10, 30, 5),
                    "by_model": [{"model": "opus", "tokens": tokens(10, 30, 5)}],
                    "by_who": [{"who": "session", "tokens": tokens(10, 30, 5)}],
                    "cache_hit": 0.75, "last_refusal": refusal,
                    "on_credits": tokens(10, 30, 5),
                    "most_seen_before_refusal": tokens(0, 0, 0),
                    "allowance": {"kind": "reported", "utilization": 0.5, "resets_at": null, "as_of": "2026-09-12T11:00:00Z"},
                    "elsewhere": tokens(0, 0, 0),
                },
                "seven_day": {
                    "window": "seven_day", "since": "2026-09-05T12:00:00Z", "requests": 2,
                    "tokens": tokens(11, 30, 6),
                    "by_model": [{"model": "opus", "tokens": tokens(11, 30, 6)}],
                    "by_who": [
                        {"who": "session", "tokens": tokens(10, 30, 5)},
                        {"who": "automated", "tokens": tokens(1, 0, 1)},
                    ],
                    "cache_hit": 30.0 / 41.0, "last_refusal": null, "on_credits": null,
                    "most_seen_before_refusal": null,
                    "allowance": {"kind": "rate_limited", "since": "2026-09-12T08:00:00Z", "retry_after_seconds": 60, "last_reported": reported},
                    "elsewhere": tokens(0, 0, 0),
                },
                "seen": tokens(11, 30, 6),
                "seen_requests": 2,
                "seen_since": "2026-09-10T12:00:00Z",
                "scope": {"counted": [], "not_counted": []},
            })
        );
    }
}

//! Token and dollar arithmetic, in one place so `bench run` and `bench cost` print the same
//! numbers for the same run.
//!
//! Prices are an input the reader supplies: [`Prices`] has no `Default` and no built-in
//! table, because a baked-in price list is a claim about the market this program cannot
//! keep true.

use serde::{Deserialize, Serialize};
use suzatary_core::capability::TokenUsage;

/// Token totals across a run or a subset of it; the score report carries one per model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Totals {
    /// How many answers this totalled over.
    pub answers: usize,
    /// How many of those carried a [`TokenUsage`]; never assumed to be every answer.
    pub reporting: usize,
    /// Summed prompt tokens, over every answer that reported one.
    pub prompt_tokens: u64,
    /// Summed completion tokens.
    pub completion_tokens: u64,
    /// Summed cached-prompt tokens; `None` when nothing reported any, never a zero standing
    /// in for "not measured".
    pub cached_prompt_tokens: Option<u64>,
}

impl Totals {
    /// Sum every answer's usage: a `None` answer counts toward `answers` but not
    /// `reporting`, and `cached_prompt_tokens` stays `None` unless one answer reported it.
    #[must_use]
    pub fn of<'a>(usages: impl Iterator<Item = &'a Option<TokenUsage>>) -> Self {
        let mut totals = Self::default();
        let mut any_cached = false;
        for usage in usages {
            totals.answers += 1;
            let Some(usage) = usage else { continue };
            totals.reporting += 1;
            totals.prompt_tokens += usage.prompt_tokens;
            totals.completion_tokens += usage.completion_tokens;
            if let Some(cached) = usage.cached_prompt_tokens {
                any_cached = true;
                totals.cached_prompt_tokens =
                    Some(totals.cached_prompt_tokens.unwrap_or(0) + cached);
            }
        }
        if !any_cached {
            totals.cached_prompt_tokens = None;
        }
        totals
    }

    /// The dollar cost, over prices the caller supplies.
    #[must_use]
    pub fn dollars(&self, prices: Prices) -> f64 {
        let million = 1_000_000.0;
        #[allow(clippy::cast_precision_loss)]
        let prompt = self.prompt_tokens as f64 / million * prices.usd_per_million_prompt;
        #[allow(clippy::cast_precision_loss)]
        let completion =
            self.completion_tokens as f64 / million * prices.usd_per_million_completion;
        prompt + completion
    }
}

/// `--price-in`/`--price-out`: US dollars per million tokens, the unit vendors publish.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Prices {
    /// Dollars per million prompt tokens.
    pub usd_per_million_prompt: f64,
    /// Dollars per million completion tokens.
    pub usd_per_million_completion: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(prompt: u64, completion: u64, cached: Option<u64>) -> TokenUsage {
        TokenUsage {
            prompt_tokens: prompt,
            completion_tokens: completion,
            cached_prompt_tokens: cached,
        }
    }

    /// A silent answer counts toward `answers` but not `reporting`.
    #[test]
    fn totals_sum_reported_answers_and_count_silent_ones() {
        let usages = [
            Some(usage(100, 10, Some(5))),
            None,
            Some(usage(50, 5, None)),
        ];
        let totals = Totals::of(usages.iter());
        assert_eq!(totals.answers, 3);
        assert_eq!(totals.reporting, 2);
        assert_eq!(totals.prompt_tokens, 150);
        assert_eq!(totals.completion_tokens, 15);
        assert_eq!(totals.cached_prompt_tokens, Some(5));
    }

    /// No answer reporting caching leaves `cached_prompt_tokens` `None`, never zero.
    #[test]
    fn no_answer_reporting_caching_is_none() {
        let usages = [Some(usage(100, 10, None)), Some(usage(50, 5, None))];
        let totals = Totals::of(usages.iter());
        assert!(totals.cached_prompt_tokens.is_none());
    }

    /// An empty run totals to zero, not a panic.
    #[test]
    fn an_empty_run_totals_to_zero() {
        let totals = Totals::of(std::iter::empty());
        assert_eq!(totals, Totals::default());
    }

    /// Dollars are prompt plus completion, each at its own price.
    #[test]
    fn dollars_price_prompt_and_completion_separately() {
        let usages = [Some(usage(1_000_000, 500_000, None))];
        let totals = Totals::of(usages.iter());
        let dollars = totals.dollars(Prices {
            usd_per_million_prompt: 3.0,
            usd_per_million_completion: 15.0,
        });
        assert!((dollars - (3.0 + 7.5)).abs() < 1e-9, "{dollars}");
    }

    /// Zero tokens price to zero dollars, whatever the price.
    #[test]
    fn zero_tokens_price_to_zero_dollars() {
        let totals = Totals::default();
        let dollars = totals.dollars(Prices {
            usd_per_million_prompt: 100.0,
            usd_per_million_completion: 100.0,
        });
        assert!(dollars.abs() < 1e-9, "{dollars}");
    }
}

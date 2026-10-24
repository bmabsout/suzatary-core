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

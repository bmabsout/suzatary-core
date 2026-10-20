//! Each Google account's calendars, read through Google's Calendar API into the calendar's
//! cache tier: what `suzatary plugin sync` runs beside the plugins, on the same timer.

use suzatary_core::capability::Clock as _;
use suzatary_core::mail::account::Sync;
use suzatary_core::mail::address::EmailAddress;

use crate::instance::Instance;

/// Sync every Google account's calendars: one line each, and whether every one was stored.
#[must_use]
pub fn sync_calendars(instance: &Instance) -> (Vec<String>, bool) {
    let grants = super::mail::google_grants(instance);
    let now = suzatary_io::clock::SystemClock.now();
    let mut stored = true;
    let lines = instance
        .mail
        .accounts
        .accounts()
        .iter()
        .filter(|account| matches!(account.sync, Sync::Google))
        .map(|account| {
            let said = EmailAddress::new(account.email.as_str())
                .map_err(|refused| refused.message().to_owned())
                .and_then(|address| grants.live(&address).map_err(|failed| failed.to_string()))
                .and_then(|grant| {
                    if !grant.parts.contains(suzatary_core::google::Scope::Calendar) {
                        return Err(
                            "its grant holds no calendars: a sign-in card asks Google for them"
                                .to_owned(),
                        );
                    }
                    grants
                        .google()
                        .occurrences(&grant, prodrome_core::fpl::instant_of(now))
                        .map_err(|failed| failed.to_string())
                })
                .and_then(|occurrences| {
                    suzatary_io::google::learn(
                        &instance.google_calendar(account),
                        &occurrences,
                        now,
                    )
                    .map(|learnt| (occurrences.len(), learnt))
                });
            match said {
                Ok((read, learnt)) => format!(
                    "google {}: {read} occurrences read, {} learned, {} known",
                    account.email, learnt.written, learnt.known
                ),
                Err(why) => {
                    stored = false;
                    format!("google {}: {why}", account.email)
                }
            }
        })
        .collect();
    (lines, stored)
}

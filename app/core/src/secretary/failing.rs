//! The rule over the connections' health: what a failing connection is owed, by its failure's
//! class, and what an unused one is offered, filed by the box with no model asked.
//!
//! This generalises `mail_health`'s identity rule to every connection: a fix is a function of
//! the connection and the instant its failure began (its last success, or its first result), so
//! seeing the same failure again asks for the same card, which stays as it was answered, and a
//! failure after a success is a new episode with a new card. A class has one remedy
//! ([`Failure::remedy`]); the card is that remedy's step of the connection's own template,
//! except that a failing link is always owed its Secret card, worded by its class, since
//! setting a link again is a valid move whatever went wrong.
//!
//! What goes unused is offered for removal, each a card the owner may dismiss: a declared
//! provider [idle] for [`IDLE`] days (its Configure card, a move to nothing), a calendar
//! whose link syncs and holds nothing within [`QUIET`] of today, and a mail account failing for
//! [`ABANDONED`]. A connection whose failure a card already mends is offered no removal beside
//! that card.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Duration;

use crate::calendar::CalendarName;
use crate::connect::{Connection, Declared, Failure, Key, Remedy, Step, Table};
use crate::mail::health::{Health, Healths, Instant, Reading};
use crate::models::Backend;
use crate::plugin::PluginName;
use crate::secretary::acts::Act;
use crate::secretary::bootstrap::Answering;
use crate::secretary::inbox::Fold;
use crate::secretary::noticed::Owed;
use crate::secretary::responded::{Done, Responded};
use crate::secretary::store::Proposal;
use crate::settings::configure::{Configure, Configured};
use crate::settings::{DeclaredProvider, Settable, Settables};

/// How long a calendar may hold nothing, before today and after it, before it is offered for
/// removal.
#[must_use]
pub fn quiet() -> Duration {
    Duration::days(i64::from(QUIET))
}

/// [`quiet`], in days.
pub const QUIET: u32 = 90;

/// How long a mail account may fail before it is offered for removal.
#[must_use]
pub fn abandoned() -> Duration {
    Duration::days(ABANDONED)
}

/// [`abandoned`], in days.
pub const ABANDONED: i64 = 7;

/// How long a declared provider may go unused before it is offered for removal.
#[must_use]
pub fn unused_for() -> Duration {
    Duration::days(IDLE)
}

/// [`unused_for`], in days.
pub const IDLE: i64 = 7;

/// Whether the provider `name` is idle at `now`: no job's order nor the fallback names it, and
/// neither a tapped card declared it nor did any of its models answer within [`IDLE`] days. Its
/// key is asked for only while it is not idle, and it is offered for removal only once it is,
/// so a provider just added from a card is asked for its key at once and never offered back.
#[must_use]
pub fn idle(answering: &Answering<'_>, inbox: &Fold, name: &str, now: Instant) -> bool {
    if answering.models.orders(name)
        || answering.fallback.backend == Backend::Declared(name.to_owned())
    {
        return false;
    }
    let prefix = format!("{name}/");
    let answered = answering
        .health
        .readings()
        .filter(|(entry, _)| entry.starts_with(&prefix))
        .map(|(_, reading)| match reading {
            Reading::Ok { at } => at,
            Reading::Failing { since, .. } => since,
        });
    let declared = inbox.objects().filter_map(|object| {
        let Proposal::Act(Act::Configure { change }) = &object.asked.proposal else {
            return None;
        };
        (change.declares() == Some(name))
            .then(|| object.reading.shown())
            .flatten()
            .filter(|answer| matches!(answer.response, Responded::Done(Done::Acted { .. })))
            .map(|answer| prodrome_core::fpl::instant_of(answer.at))
    });
    answered
        .chain(declared)
        .max()
        .is_none_or(|last| now - last >= unused_for())
}

/// What the rule reads.
#[derive(Clone, Copy)]
pub struct Observed<'a> {
    /// What the inbox holds: when a provider was declared from a card.
    pub inbox: &'a Fold,
    /// The tables connections are read from.
    pub declared: &'a Declared,
    /// What the models answer, and every secret name declared.
    pub answering: &'a Answering<'a>,
    /// Every `[[providers]]` entry.
    pub providers: &'a [DeclaredProvider],
    /// Each mail account's syncs.
    pub mail: &'a Healths,
    /// Each plugin's links' fetches, by plugin.
    pub links: &'a BTreeMap<PluginName, Healths>,
    /// The calendars holding an occurrence within [`quiet`] of `now`; `None` when the
    /// calendars could not be read, which holds every waiting card, since a calendar still
    /// quiet cannot be told from one that is not.
    pub heard: Option<&'a BTreeSet<CalendarName>>,
    /// Now, UTC.
    pub now: Instant,
}

/// What a failing connection is, as its fix needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A connection the instance's tables hold.
    Connection(Connection),
    /// A model entry a job may answer with, by its print, and the secret its backend reads.
    Model {
        /// `claude-cli/<model>`, `<provider>/<model>`.
        entry: String,
        /// The key or token its backend reads, where one is declared.
        key: Option<Settable>,
    },
}

/// One failing observation: what failed, of what class, since when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failing {
    /// What failed.
    pub target: Target,
    /// Its class.
    pub class: Failure,
    /// It has not succeeded since this instant.
    pub since: Instant,
}

impl Failing {
    /// The one card this failure is owed: its class's remedy as a step of its own template
    /// (the link's secret, the account's sign-in) or its backend's key; none for a remedy no
    /// tap gives, or one its connection has no step for.
    #[must_use]
    pub fn fix(&self, settables: &Settables) -> Option<Owed> {
        let episode = prodrome_core::fpl::datetime_of(self.since).ok()?;
        let since = utc(self.since);
        let act = match (self.target.remedy(self.class), &self.target) {
            (Remedy::Retry, Target::Connection(_) | Target::Model { .. })
            | (Remedy::Auth, Target::Model { .. }) => return None,
            (Remedy::Secret, Target::Connection(connection)) => {
                let name = connection.secrets().into_iter().next()?;
                Act::Secret {
                    name: Settable::mint(settables, &name).ok()?.name().to_owned(),
                    // The same words whatever the class, so a class moving within one
                    // episode (an outage, then 401) asks for the one card; the class is said
                    // in the line above it.
                    why: format!(
                        "{} has failed since {since}: store it again",
                        connection.called()
                    ),
                }
            }
            (Remedy::Secret, Target::Model { entry, key }) => Act::Secret {
                name: key.as_ref()?.name().to_owned(),
                why: format!("{entry} has failed since {since}: set its key again"),
            },
            (Remedy::Auth, Target::Connection(connection)) => {
                connection.steps().into_iter().find_map(|step| match step {
                    Step::DeviceCode { account } => Some(Act::Auth { account }),
                    // A revoked grant, or one a client in Testing ended after 7 days, holds no
                    // part any more (Google forgets the whole grant), so every part is missing
                    // and the consent asks for them all again.
                    Step::AuthCode { account } => Some(Act::GoogleSignIn {
                        account,
                        scopes: crate::google::Scopes::all(),
                    }),
                    Step::Secret { .. }
                    | Step::SendingConsent { .. }
                    | Step::GoogleClient
                    | Step::SignIn
                    | Step::Terminal { .. } => None,
                })?
            }
        };
        Some(
            Owed::one(
                Proposal::Act(act),
                format!(
                    "{} has failed since {since}: {}",
                    self.named(),
                    said(self.class)
                ),
            )
            .since(episode),
        )
    }

    /// What failed, as the owner calls it.
    fn named(&self) -> String {
        match &self.target {
            Target::Connection(connection) => connection.called(),
            Target::Model { entry, .. } => format!("the model {entry}"),
        }
    }
}

impl Target {
    /// What mends `class` here: a link is always set again from its Secret card, since setting
    /// it again is a valid move whatever its failure (a mistyped address, a rotated link, a
    /// service gone); everything else takes its class's one remedy.
    fn remedy(&self, class: Failure) -> Remedy {
        match self {
            Target::Connection(connection) => match connection.template().table {
                Table::Link(_) => Remedy::Secret,
                Table::Always
                | Table::Backend(_)
                | Table::Account(_)
                | Table::Folder
                | Table::Channel(_) => class.remedy(),
            },
            Target::Model { .. } => class.remedy(),
        }
    }
}

/// `words` begun with a capital, as a sentence begins.
fn capitalised(words: &str) -> String {
    let mut chars = words.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// An instant as a card says it: `2026-10-07 09:00 UTC`.
fn utc(at: Instant) -> String {
    format!("{} UTC", at.format("%Y-%m-%d %H:%M"))
}

/// What a class of failure said, in the box's words: the line the box says above a card it
/// files, never the card's own identity.
fn said(class: Failure) -> String {
    match class {
        Failure::Revoked { status } => format!(
            "it answered {}, so its link was changed or revoked",
            status.status()
        ),
        Failure::NeedsLogin => "it needs its sign-in again".to_owned(),
        Failure::Unresolved => "it reads a secret stored nowhere".to_owned(),
        Failure::KeyRefused => "its key was refused".to_owned(),
        Failure::Unusable => "the link stored for it is not one the box sends".to_owned(),
        Failure::Unreachable => {
            "its address could not be reached: check the link and set it again".to_owned()
        }
        Failure::Unclassified => "it failed".to_owned(),
    }
}

/// Every failing observation: each link of a declared plugin, each mail account and, while some
/// model answers, each model entry a job may answer with, whose results read as failing.
#[must_use]
pub fn failing(observed: &Observed<'_>) -> Vec<Failing> {
    let connections = observed.declared.connections();
    let of = |health: Option<&Health>| health.and_then(Health::remedial);
    let connected = connections.values().filter_map(|connection| {
        let reading = match connection.template().table {
            Table::Link(fragment) => {
                let name = connection.secrets().into_iter().next()?;
                observed
                    .links
                    .iter()
                    .find(|(plugin, _)| plugin.as_str() == fragment.plugin)
                    .and_then(|(_, links)| links.health(&name))
            }
            Table::Account(_) => observed.mail.health(&connection.key().name),
            Table::Always | Table::Backend(_) | Table::Folder | Table::Channel(_) => None,
        };
        let (class, since) = of(reading)?;
        Some(Failing {
            target: Target::Connection(connection.clone()),
            class,
            since,
        })
    });
    // While no model answers, connecting one is the bootstrap's to ask (`bootstrap::owed`).
    let models = observed
        .answering
        .entries()
        .into_iter()
        .filter(|_| observed.answering.any())
        .filter_map(|entry| {
            let (class, since) = of(observed.answering.health.health(&entry.to_string()))?;
            Some(Failing {
                target: Target::Model {
                    entry: entry.to_string(),
                    key: observed.answering.credential(&entry.backend),
                },
                class,
                since,
            })
        });
    connected.chain(models).collect()
}

/// The cards the connections' health owes, besides what `asked` already asks for: one per
/// failing connection and episode whose remedy a tap gives, a secret or a sign-in another rule
/// already asks for being that card; and each removal an unused connection is offered.
#[must_use]
pub fn owed(observed: &Observed<'_>, asked: &Owed) -> Owed {
    let settables = observed.answering.settables;
    let asks = |proposal: &Proposal| {
        asked
            .iter()
            .any(|noticed| match (&noticed.proposal, proposal) {
                (
                    Proposal::Act(Act::Secret { name: held, .. }),
                    Proposal::Act(Act::Secret { name, .. }),
                ) => held == name,
                (held, proposal) => held == proposal,
            })
    };
    let failing = failing(observed);
    let fixes = failing
        .iter()
        .filter_map(|failing| failing.fix(settables))
        .filter(|owed| !owed.iter().any(|noticed| asks(&noticed.proposal)))
        .fold(Owed::default(), Owed::join);
    // A failure a tap mends has its card, from this rule or another; removing the connection
    // beside it would offer the opposite act for the same state.
    let mended: BTreeSet<Key> = failing
        .iter()
        .filter(|failing| failing.class.remedy() != Remedy::Retry)
        .filter_map(|failing| match &failing.target {
            Target::Connection(connection) => Some(connection.key()),
            Target::Model { .. } => None,
        })
        .collect();
    fixes.join(unused(observed, &mended))
}

/// The removals an unused connection is offered, none for a connection in `mended`.
fn unused(observed: &Observed<'_>, mended: &BTreeSet<Key>) -> Owed {
    let answering = observed.answering;
    // Removing a provider is the provider's Configure card (stage 5): a move to nothing.
    let configured = Configured {
        models: answering.models.clone(),
        providers: observed.providers.to_vec(),
    };
    let providers = observed
        .providers
        .iter()
        .filter(|provider| idle(answering, observed.inbox, &provider.name, observed.now))
        .filter_map(|provider| {
            let change = Configure::remove(&configured, &provider.name).ok()?;
            Some(Owed::one(
                Proposal::Act(Act::Configure { change }),
                format!(
                    "No job has used {} for {IDLE} days or more.",
                    crate::settings::configure::provider_label(&provider.name)
                ),
            ))
        });
    let connections = observed.declared.connections();
    let accounts = connections
        .values()
        .filter(|connection| !mended.contains(&connection.key()))
        .filter_map(|connection| {
            match connection.template().table {
                Table::Account(_) => {}
                Table::Always
                | Table::Backend(_)
                | Table::Link(_)
                | Table::Folder
                | Table::Channel(_) => return None,
            }
            let reading = observed.mail.reading(&connection.key().name)?;
            let since = match reading {
                Reading::Failing { since, .. }
                    if reading.failing_for(observed.now) >= Some(abandoned()) =>
                {
                    since
                }
                Reading::Failing { .. } | Reading::Ok { .. } => return None,
            };
            Some(
                Owed::one(
                    Proposal::Act(Act::Disconnect {
                        connection: connection.clone(),
                    }),
                    format!(
                        "{} has not synced since {}, {ABANDONED} days or more.",
                        connection.called(),
                        utc(since)
                    ),
                )
                .since(prodrome_core::fpl::datetime_of(since).ok()?),
            )
        });
    let calendars = match observed.heard {
        None => Owed::default().holding(),
        Some(heard) => connections
            .values()
            .filter(|connection| !mended.contains(&connection.key()))
            .filter_map(|connection| quiet_calendar(observed, heard, connection))
            .fold(Owed::default(), Owed::join),
    };
    providers
        .chain(accounts)
        .fold(Owed::default(), Owed::join)
        .join(calendars)
}

/// The removal a calendar is offered when its link syncs, its latest fetch included, and its
/// store holds nothing within [`quiet`] of now: a calendar that holds nothing because its link
/// fails is offered none.
fn quiet_calendar(
    observed: &Observed<'_>,
    heard: &BTreeSet<CalendarName>,
    connection: &Connection,
) -> Option<Owed> {
    let fragment = match connection.template().table {
        Table::Link(fragment) => fragment,
        Table::Always
        | Table::Backend(_)
        | Table::Account(_)
        | Table::Folder
        | Table::Channel(_) => return None,
    };
    let name = connection.secrets().into_iter().next()?;
    let synced = observed
        .links
        .iter()
        .find(|(plugin, _)| plugin.as_str() == fragment.plugin)
        .and_then(|(_, links)| links.health(&name))
        .and_then(|health| match health.reading() {
            Reading::Ok { at } => Some(at),
            Reading::Failing { .. } => None,
        })?;
    if heard.iter().any(|calendar| calendar.as_str() == name) {
        return None;
    }
    Some(Owed::one(
        Proposal::Act(Act::Disconnect {
            connection: connection.clone(),
        }),
        format!(
            "{} synced last at {} and holds nothing within {QUIET} days of today.",
            capitalised(&connection.called()),
            utc(synced)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail::health::Outcome;
    use crate::models::{Job, ModelEntry, Models};
    use crate::secretary::inbox::{self, Fold};
    use crate::secretary::inbox_event::InboxEvent;
    use crate::secretary::proposer::Proposer;
    use crate::store::InMemory;

    fn at(hour: u32) -> Instant {
        chrono::NaiveDate::from_ymd_opt(2026, 10, 1)
            .and_then(|day| day.and_hms_opt(0, 0, 0))
            .expect("an instant")
            + Duration::hours(i64::from(hour))
    }

    fn work() -> Connection {
        Connection::parse("ics", &["Work".to_owned()]).expect("a calendar")
    }

    fn account() -> Connection {
        Connection::parse(
            "imap-device-code",
            &["ada@example.test".to_owned(), "consumers".to_owned()],
        )
        .expect("an account")
    }

    fn ics() -> PluginName {
        PluginName::parse("ics").expect("a name")
    }

    /// A deployment declaring the Work calendar and Ada's Microsoft account, and the providers
    /// named, none ordered.
    struct World {
        declared: Declared,
        providers: Vec<crate::settings::DeclaredProvider>,
        settables: Settables,
        models: Models,
        fallback: ModelEntry,
        answered: Healths,
        mail: Healths,
        links: BTreeMap<PluginName, Healths>,
        heard: BTreeSet<CalendarName>,
        inbox: Fold,
    }

    impl World {
        fn new(providers: &[&str]) -> Self {
            let declared = Declared::default()
                .with(&work())
                .and_then(|declared| declared.with(&account()))
                .expect("declared");
            let providers: Vec<crate::settings::DeclaredProvider> = providers
                .iter()
                .map(|name| crate::settings::DeclaredProvider {
                    name: (*name).to_owned(),
                    base_url: format!("https://{name}.example.test/v1"),
                    key: format!("{}_API_KEY", name.to_uppercase()),
                    attestation: crate::settings::Attestation::Unclaimed,
                })
                .collect();
            Self {
                settables: Settables::derive(providers.iter(), [], None, &declared.plugins),
                providers,
                declared,
                models: Models::default(),
                fallback: ModelEntry::parse(&[], "claude-cli/a-model").expect("an entry"),
                answered: Healths::default(),
                mail: Healths::default(),
                links: BTreeMap::new(),
                heard: BTreeSet::new(),
                inbox: Fold::default(),
            }
        }

        fn link(&mut self, at: Instant, outcome: Outcome) {
            let joined = self
                .links
                .get(&ics())
                .cloned()
                .unwrap_or_default()
                .join(&Healths::one("PLUGIN_ICS_WORK", at, outcome, "it answered"));
            self.links.insert(ics(), joined);
        }

        fn owed(&self, now: Instant) -> Owed {
            let resolves = |_: &Settable| true;
            let answering = Answering {
                models: &self.models,
                fallback: &self.fallback,
                health: &self.answered,
                settables: &self.settables,
                resolves: &resolves,
            };
            owed(
                &Observed {
                    inbox: &self.inbox,
                    declared: &self.declared,
                    answering: &answering,
                    providers: &self.providers,
                    mail: &self.mail,
                    links: &self.links,
                    heard: Some(&self.heard),
                    now,
                },
                &Owed::default(),
            )
        }
    }

    fn acts(owed: &Owed) -> Vec<Act> {
        owed.iter()
            .filter_map(|noticed| match &noticed.proposal {
                Proposal::Act(act) => Some(act.clone()),
                _ => None,
            })
            .collect()
    }

    /// The act a remedy is, by its kind: what the law compares.
    fn kind(remedy: Remedy) -> Option<&'static str> {
        match remedy {
            Remedy::Secret => Some("secret"),
            Remedy::Auth => Some("auth"),
            Remedy::Retry => None,
        }
    }

    /// Every class has exactly one remedy, and a failing connection of a class is owed exactly
    /// the act its remedy names, of the step its template has for it: an account's sign-in, a
    /// model's key; a retry is owed none. A link is owed its Secret card whatever the class,
    /// an address that could not be reached included.
    #[test]
    fn every_failure_class_maps_to_exactly_one_fix_act() {
        let world = World::new(&[]);
        let since = at(1);
        for class in Failure::ALL {
            let targets = [
                Target::Connection(work()),
                Target::Connection(account()),
                Target::Model {
                    entry: "claude-cli/a-model".to_owned(),
                    key: Settable::mint(&world.settables, "CLAUDE_CODE_OAUTH_TOKEN").ok(),
                },
            ];
            let fixed: Vec<Vec<Act>> = targets
                .into_iter()
                .map(|target| {
                    Failing {
                        target,
                        class,
                        since,
                    }
                    .fix(&world.settables)
                    .map(|owed| acts(&owed))
                    .unwrap_or_default()
                })
                .collect();
            assert!(
                fixed.iter().all(|acts| acts.len() <= 1),
                "{class:?}: {fixed:?}"
            );
            // A link is set again whatever its class; the others take their class's remedy.
            let [link, others @ ..] = &fixed[..] else {
                panic!("three targets");
            };
            assert!(
                matches!(&link[..], [Act::Secret { name, .. }] if name == "PLUGIN_ICS_WORK"),
                "{class:?}: {link:?}"
            );
            let kinds: BTreeSet<&str> = others.iter().flatten().map(Act::kind).collect();
            assert_eq!(
                kinds.into_iter().next(),
                kind(class.remedy()),
                "{class:?} is fixed by its remedy alone"
            );
        }
        let revoked = Failing {
            target: Target::Connection(work()),
            class: Failure::of_status(401),
            since,
        };
        assert!(matches!(
            acts(&revoked.fix(&world.settables).expect("a card"))[..],
            [Act::Secret { ref name, .. }] if name == "PLUGIN_ICS_WORK"
        ));
        let lapsed = Failing {
            target: Target::Connection(account()),
            class: Failure::NeedsLogin,
            since,
        };
        assert!(matches!(
            acts(&lapsed.fix(&world.settables).expect("a card"))[..],
            [Act::Auth { ref account }] if account.as_str() == "ada@example.test"
        ));
    }

    /// A Google account whose grant Google refused (revoked, or ended after 7 days by a client
    /// in Testing) is owed its sign-in with Google again, for every part; a missing client is
    /// the connections rule's card, so no fix of a class asks for it.
    #[test]
    fn a_google_account_needing_its_sign_in_is_owed_the_sign_in_with_google() {
        let world = World::new(&[]);
        let google = Connection::parse("google", &["ada@example.test".to_owned()])
            .expect("a Google account");
        let fixed = |class| {
            Failing {
                target: Target::Connection(google.clone()),
                class,
                since: at(1),
            }
            .fix(&world.settables)
            .map(|owed| acts(&owed))
            .unwrap_or_default()
        };
        assert!(matches!(
            &fixed(Failure::NeedsLogin)[..],
            [Act::GoogleSignIn { account, scopes }]
                if account.as_str() == "ada@example.test"
                    && *scopes == crate::google::Scopes::all()
        ));
        for class in [Failure::Unresolved, Failure::Unclassified] {
            assert!(fixed(class).is_empty(), "{class:?}");
        }
    }

    fn file(
        owed: &Owed,
        inbox_store: &InMemory<InboxEvent>,
        conversations: &crate::secretary::conversations::InMemory,
        hour: u32,
    ) {
        owed.file(
            conversations.conversations(),
            inbox_store,
            prodrome_core::fpl::datetime_of(at(hour)).expect("in range"),
        )
        .expect("filed");
    }

    fn secrets_filed(fold: &Fold) -> usize {
        fold.objects()
            .filter(|object| {
                object.asked.proposer == Proposer::Box
                    && matches!(&object.asked.proposal, Proposal::Act(Act::Secret { .. }))
            })
            .count()
    }

    /// One result of a link: it synced, or it failed of a refusing status, of an address not
    /// reached, or of an outage.
    #[derive(Debug, Clone, Copy)]
    enum Seen {
        Synced,
        Refused(u16),
        Unreached,
        Outage,
    }

    fn seen() -> impl proptest::strategy::Strategy<Value = Seen> {
        use proptest::prelude::*;
        prop_oneof![
            Just(Seen::Synced),
            proptest::sample::select(vec![401u16, 403, 404]).prop_map(Seen::Refused),
            Just(Seen::Unreached),
            Just(Seen::Outage),
        ]
    }

    proptest::proptest! {
        /// However often each failure is seen, whatever its class moves to between sightings
        /// (an outage, 403, then 404), and whatever the box restarts between filings (each
        /// reads the inbox afresh), one card is filed per episode of a failing link, an episode
        /// being every result since one success; and the card stays as it stood (dismissed stays
        /// dismissed, waiting stays waiting) until a success.
        #[test]
        fn one_act_per_connection_and_since(
            script in proptest::collection::vec((seen(), 1u8..4, proptest::bool::ANY), 1..20)
        ) {
            let mut world = World::new(&[]);
            let inbox_store = InMemory::<InboxEvent>::default();
            let conversations = crate::secretary::conversations::InMemory::default();
            // How many episodes began.
            let mut episodes = 0usize;
            let mut open_episode = false;
            let mut hour = 0;
            for (result, sightings, dismiss) in script {
                hour += 1;
                world.link(at(hour), outcome(result));
                match result {
                    Seen::Synced => open_episode = false,
                    Seen::Refused(_) | Seen::Unreached | Seen::Outage => {
                        // A failure after a success, or the first result, begins an episode.
                        if !open_episode {
                            episodes += 1;
                            open_episode = true;
                        }
                    }
                }
                for _ in 0..sightings {
                    file(&world.owed(at(hour)), &inbox_store, &conversations, hour);
                }
                let fold = inbox::fold(&inbox_store).expect("reads");
                let waiting = fold.objects().filter(|object| {
                    object.reading.open()
                        && matches!(&object.asked.proposal, Proposal::Act(Act::Secret { .. }))
                }).count();
                // Within an episode, its card waits until dismissed or a success.
                proptest::prop_assert!(waiting <= 1, "{} cards wait at once", waiting);
                if dismiss
                    && let Some(open) = fold.objects().find(|object| object.reading.open())
                {
                    let owner = prodrome_core::event::Actor::new("ada").expect("an actor");
                    let when = prodrome_core::fpl::datetime_of(at(hour)).expect("in range");
                    inbox::dismiss(&inbox_store, &open.id, &owner, when, "not now").expect("dismissed");
                }
            }
            let fold = inbox::fold(&inbox_store).expect("reads");
            proptest::prop_assert_eq!(secrets_filed(&fold), episodes);
            // A quiet calendar's removal is withdrawn once its link fails, since a fix card
            // stands then; only the fix cards are counted here.
            let withdrawn = fold.objects().filter(|object| {
                matches!(&object.asked.proposal, Proposal::Act(Act::Secret { .. }))
                && matches!(object.reading.shown().map(|answer| &answer.response),
                    Some(crate::secretary::responded::Responded::Withdrawn(_)))
            }).count();
            // Only an episode a success ended withdraws its card.
            proptest::prop_assert!(withdrawn <= episodes.saturating_sub(usize::from(open_episode)));
        }
    }

    fn outcome(result: Seen) -> Outcome {
        match result {
            Seen::Synced => Outcome::Succeeded,
            Seen::Refused(status) => Outcome::Failed(Failure::of_status(status)),
            Seen::Unreached => Outcome::Failed(Failure::Unreachable),
            Seen::Outage => Outcome::Failed(Failure::Unclassified),
        }
    }

    /// A link whose address could not be reached, the class with no remedy of its own, is owed
    /// its Secret card, asking to check the link and set it again.
    #[test]
    fn a_link_not_reached_is_owed_its_secret_card() {
        let mut world = World::new(&[]);
        world.link(at(1), Outcome::Failed(Failure::Unreachable));
        let owed = world.owed(at(2));
        let said: Vec<&str> = owed.iter().map(|noticed| noticed.why.as_str()).collect();
        assert!(
            matches!(&acts(&owed)[..], [Act::Secret { name, .. }] if name == "PLUGIN_ICS_WORK"),
            "{owed:?}"
        );
        assert!(
            said.iter()
                .any(|why| why.contains("could not be reached") && why.contains("set it again")),
            "worded by its class: {said:?}"
        );
        assert!(
            said.iter().all(|why| !why.contains("lost")),
            "said in the owner's words: {said:?}"
        );
    }

    /// A model whose key was refused is owed a card for that key while another model answers;
    /// while none does, the bootstrap asks for a model, and this rule asks nothing.
    #[test]
    fn a_refused_key_is_asked_for_while_another_model_answers() {
        let mut world = World::new(&[]);
        let refused = Healths::one(
            "claude-cli/a-model",
            at(1),
            Outcome::Failed(Failure::KeyRefused),
            "401",
        );
        world.answered = refused.clone();
        assert!(
            !acts(&world.owed(at(2)))
                .iter()
                .any(|act| matches!(act, Act::Secret { .. }))
        );
        world.models = Models::default().with_order(
            Job::Review,
            vec![ModelEntry::parse(&[], "anthropic/b-model").expect("an entry")],
        );
        world.answered = refused.join(&Healths::one(
            "anthropic/b-model",
            at(1),
            Outcome::Succeeded,
            "",
        ));
        assert!(acts(&world.owed(at(2))).iter().any(|act| matches!(
            act,
            Act::Secret { name, .. } if name == "CLAUDE_CODE_OAUTH_TOKEN"
        )));
    }

    /// A provider declared from a card is not idle for a week after the tap: its key is asked
    /// for at once and it is not offered back; a week on, unused, it is offered for removal.
    #[test]
    fn a_provider_declared_from_a_card_is_kept_a_week() {
        let mut world = World::new(&["openrouter"]);
        let declare = crate::settings::configure::Configure::declare(
            &crate::settings::configure::Configured::default(),
            crate::settings::ProviderSetup::parse(crate::settings::Preset::OpenRouter, None, None)
                .expect("a preset"),
        )
        .expect("a change");
        assert_eq!(declare.declares(), Some("openrouter"));
        let store = InMemory::<InboxEvent>::default();
        let asked = inbox::Asked {
            proposer: Proposer::Secretary,
            proposal: Proposal::Act(Act::Configure { change: declare }),
            reasons: crate::secretary::store::Reasons::new(
                "Ada asked",
                vec!["briefing:2026-10-01".parse().expect("a reference")],
            )
            .expect("reasons"),
            asked: "proposal:models/0".parse().expect("a reference"),
            price: inbox::Price::unpriced(),
        };
        let tapped = prodrome_core::fpl::datetime_of(at(1)).expect("in range");
        let id = inbox::ask(&store, asked, &crate::secretary::writes_as(), tapped).expect("asked");
        let owner = prodrome_core::event::Actor::new("ada").expect("an actor");
        inbox::submit(&store, &id, &owner, tapped, |_| {
            Ok(crate::secretary::responded::Performed::Done(Done::Acted {
                what: "declared".to_owned(),
            }))
        })
        .expect("taps");
        world.inbox = inbox::fold(&store).expect("reads");
        let removes = |world: &World, now| {
            acts(&world.owed(now)).iter().any(|act| {
                matches!(act, Act::Configure { change }
                    if change.setting() == crate::settings::configure::Setting::Provider("openrouter".to_owned()))
            })
        };
        let resolves = |_: &Settable| true;
        let answering = Answering {
            models: &world.models,
            fallback: &world.fallback,
            health: &world.answered,
            settables: &world.settables,
            resolves: &resolves,
        };
        assert!(
            !idle(&answering, &world.inbox, "openrouter", at(2)),
            "just declared"
        );
        assert!(
            !removes(&world, at(2)),
            "not offered back seconds after the tap"
        );
        assert!(!removes(&world, at(1 + 24 * 7 - 1)));
        assert!(idle(&answering, &world.inbox, "openrouter", at(1 + 24 * 7)));
        assert!(removes(&world, at(1 + 24 * 7)), "unused a week");
        world.answered = Healths::one("openrouter/m", at(24 * 7), Outcome::Succeeded, "");
        assert!(
            !removes(&world, at(1 + 24 * 7)),
            "a model of it answered within the week"
        );
    }

    /// A connection whose failure a card mends is offered no removal beside that card: a
    /// calendar refused with 401 holds nothing because it fails, and an account waiting a week
    /// for its sign-in has the sign-in's card.
    #[test]
    fn no_removal_stands_beside_a_fix() {
        let mut world = World::new(&[]);
        world.link(at(1), Outcome::Succeeded);
        world.link(at(2), Outcome::Failed(Failure::of_status(401)));
        let owed = acts(&world.owed(at(3)));
        assert!(
            !owed.contains(&Act::Disconnect { connection: work() }),
            "a failing calendar is not quiet: {owed:?}"
        );
        world.links.clear();
        world.link(at(1), Outcome::Succeeded);
        world.link(at(2), Outcome::Failed(Failure::Unclassified));
        assert!(
            !acts(&world.owed(at(3))).contains(&Act::Disconnect { connection: work() }),
            "nor one whose latest fetch failed for no class a tap mends"
        );
        world.mail = Healths::one(
            "ada@example.test",
            at(1),
            Outcome::Failed(Failure::NeedsLogin),
            "no sign-in",
        );
        let owed = acts(&world.owed(at(24 * 8)));
        assert!(
            owed.iter().any(|act| matches!(act, Act::Auth { .. })),
            "{owed:?}"
        );
        assert!(
            !owed.contains(&Act::Disconnect {
                connection: account()
            }),
            "{owed:?}"
        );
    }

    /// A provider no job orders is offered for removal, and once a job orders it, it is not.
    #[test]
    fn an_unordered_provider_is_offered_for_removal() {
        let mut world = World::new(&["phala"]);
        let removes = |world: &World| {
            acts(&world.owed(at(1))).iter().any(|act| {
                matches!(act, Act::Configure { change }
                    if change.setting() == crate::settings::configure::Setting::Provider("phala".to_owned()))
            })
        };
        assert!(removes(&world));
        let providers = [crate::settings::DeclaredProvider {
            name: "phala".to_owned(),
            base_url: "https://phala.example.test/v1".to_owned(),
            key: "PHALA_API_KEY".to_owned(),
            attestation: crate::settings::Attestation::Unclaimed,
        }];
        world.models = Models::default().with_order(
            Job::Review,
            vec![ModelEntry::parse(&providers, "phala/m").expect("an entry")],
        );
        assert!(!removes(&world));
    }

    /// An account failing a week is offered for removal, and one failing less is not.
    #[test]
    fn an_account_failing_a_week_is_offered_for_removal() {
        let mut world = World::new(&[]);
        world.mail =
            Healths::one("ada@example.test", at(0), Outcome::Succeeded, "").join(&Healths::one(
                "ada@example.test",
                at(1),
                Outcome::Failed(Failure::Unclassified),
                "connection refused",
            ));
        let removes = |now| {
            acts(&world.owed(now)).contains(&Act::Disconnect {
                connection: account(),
            })
        };
        assert!(!removes(at(24 * 6)));
        assert!(removes(at(24 * 7)));
    }

    /// A calendar that has synced and holds nothing within the quiet span is offered for
    /// removal; one holding something is not, nor one never synced; calendars that could not
    /// be read hold every card.
    #[test]
    fn a_quiet_calendar_is_offered_for_removal() {
        let mut world = World::new(&[]);
        let removes = |world: &World| {
            acts(&world.owed(at(2))).contains(&Act::Disconnect { connection: work() })
        };
        assert!(!removes(&world), "never synced");
        world.link(at(1), Outcome::Succeeded);
        assert!(removes(&world));
        world
            .heard
            .insert(CalendarName::parse("PLUGIN_ICS_WORK").expect("a calendar"));
        assert!(!removes(&world));
        let resolves = |_: &Settable| true;
        let answering = Answering {
            models: &world.models,
            fallback: &world.fallback,
            health: &world.answered,
            settables: &world.settables,
            resolves: &resolves,
        };
        let unread = owed(
            &Observed {
                inbox: &Fold::default(),
                declared: &world.declared,
                answering: &answering,
                providers: &world.providers,
                mail: &world.mail,
                links: &world.links,
                heard: None,
                now: at(2),
            },
            &Owed::default(),
        );
        assert_eq!(unread, Owed::default().holding());
    }

    /// A secret another rule already asks for is that card: a link that does not resolve is
    /// asked for once, by the secrets rule.
    #[test]
    fn a_secret_another_rule_asks_for_is_not_asked_twice() {
        let mut world = World::new(&[]);
        world.link(at(1), Outcome::Failed(Failure::of_status(401)));
        let asked = Owed::one(
            Proposal::Act(Act::Secret {
                name: "PLUGIN_ICS_WORK".to_owned(),
                why: "nothing is stored".to_owned(),
            }),
            "the secrets rule",
        );
        let resolves = |_: &Settable| true;
        let answering = Answering {
            models: &world.models,
            fallback: &world.fallback,
            health: &world.answered,
            settables: &world.settables,
            resolves: &resolves,
        };
        let owed = owed(
            &Observed {
                inbox: &Fold::default(),
                declared: &world.declared,
                answering: &answering,
                providers: &world.providers,
                mail: &world.mail,
                links: &world.links,
                heard: Some(&world.heard),
                now: at(2),
            },
            &asked,
        );
        assert!(
            !acts(&owed)
                .iter()
                .any(|act| matches!(act, Act::Secret { .. })),
            "{owed:?}"
        );
    }
}

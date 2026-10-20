//! The monthly Junk sweep as values. Junk is never triaged, but real mail lands
//! there, so the sweep counts headers by sender domain and lists in full only
//! what falls outside the declared churn.
//!
//! The effective churn is `seed ∪ ours − keep`. [`ChurnList`] remembers each
//! entry's [`Origin`], so a seed false positive is visible and fixed by adding
//! to `keep`, never by editing the seed. [`digest`]'s laws are property tests:
//! counts sum to the total, and no churn header is listed.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use serde::{Serialize, Serializer};

/// A domain name as a sweep compares it: lower case, no trailing dot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Domain(String);

/// Why a churn entry is not a domain.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "[mail.junk] churn: {0:?} is not a domain (letters, digits, dashes and dots, one dot at least)"
)]
pub struct NotADomain(pub String);

impl Domain {
    /// Read a domain, or say why this text is not one. Lower-cased; a
    /// trailing dot (the DNS root) dropped.
    ///
    /// # Errors
    /// [`NotADomain`] for anything but dot-separated labels of letters,
    /// digits and dashes, with two labels at least.
    pub fn parse(text: &str) -> Result<Self, NotADomain> {
        let lowered = text.trim().trim_end_matches('.').to_ascii_lowercase();
        let labels: Vec<&str> = lowered.split('.').collect();
        let label_ok = |label: &&str| {
            !label.is_empty() && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        };
        if labels.len() >= 2 && labels.iter().all(label_ok) {
            Ok(Self(lowered))
        } else {
            Err(NotADomain(text.to_owned()))
        }
    }

    /// The domain of an address, the text after its last `@`.
    #[must_use]
    pub fn of_address(address: &str) -> Option<Self> {
        address
            .rsplit_once('@')
            .and_then(|(_, domain)| Self::parse(domain).ok())
    }

    /// Whether this domain is `other` or lies under it: `mail.shop.test` is
    /// under `shop.test`, `badshop.test` is not.
    #[must_use]
    pub fn within(&self, other: &Self) -> bool {
        self.0 == other.0
            || self
                .0
                .strip_suffix(other.0.as_str())
                .is_some_and(|head| head.ends_with('.'))
    }

    /// The text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This domain, then each ancestor down to two labels: what
    /// [`within`](Self::within) treats as "at or under", so a churn lookup over a
    /// seed of hundreds of thousands is `O(depth · log n)`, not a scan.
    fn and_ancestors(&self) -> impl Iterator<Item = Self> + '_ {
        let labels: Vec<&str> = self.0.split('.').collect();
        (0..=labels.len() - 2).map(move |start| Self(labels[start..].join(".")))
    }
}

/// Where a Junk message says it came from. Spam omits or mangles `From`, so
/// a header with no readable address is its own bucket rather than a guess.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sender {
    /// The `From` address's domain.
    Domain(Domain),
    /// No address a domain could be read from.
    Unaddressed,
}

impl Sender {
    /// How the histogram prints it.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Domain(domain) => domain.as_str(),
            Self::Unaddressed => "(no address)",
        }
    }
}

impl Serialize for Sender {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Where a churn domain came from; one covered by both is labelled
/// [`Ours`](Self::Ours), the deliberate entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// From a maintained public list, normalised by [`normalize_seed_file`].
    Seed,
    /// From this deployment's own `[mail.junk] churn`.
    Ours,
}

/// `[mail.junk] keep`: domains (and their subdomains) a digest never treats
/// as churn, however the seed or `ours` lists them — the fix for a seed false
/// positive.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct KeepList(BTreeSet<Domain>);

impl KeepList {
    /// Parse the instance file's list, refusing the first entry that is not a
    /// domain.
    ///
    /// # Errors
    /// [`NotADomain`].
    pub fn new<'a>(domains: impl IntoIterator<Item = &'a str>) -> Result<Self, NotADomain> {
        domains
            .into_iter()
            .map(Domain::parse)
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// Every domain kept, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Domain> {
        self.0.iter()
    }

    /// Whether `domain` is `keep`, or lies under a `keep` entry.
    fn covers(&self, domain: &Domain) -> bool {
        domain
            .and_ancestors()
            .any(|ancestor| self.0.contains(&ancestor))
    }
}

/// Whether a digest folded a seed in, and how large. [`Seed::None`] is a real
/// state, never an error, and a digest says which population it swept.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Seed {
    /// No seed file was read.
    #[default]
    None,
    /// The seed file this deployment read, and how many domains it held.
    Loaded {
        /// How many domains the seed file held.
        domains: usize,
    },
}

/// The effective churn, `seed ∪ ours − keep`: [`Self::new`] at instance-parse
/// time, a seed folded in fresh per digest by [`Self::with_seed`] and never held
/// resident. Remembers each entry's origin so [`digest`] can label every row.
///
/// Not `Serialize`: a seed holds hundreds of thousands of domains, and printing them into a
/// verb's answer once made a 50 MB conversation line. [`Self::summary`] is what an answer says.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChurnList {
    /// Every domain this list covers, and the more deliberate origin when
    /// more than one source lists it.
    domains: BTreeMap<Domain, Origin>,
    keep: KeepList,
    seed: Seed,
}

impl ChurnList {
    /// This deployment's own `[mail.junk] churn` and `keep`, with no seed.
    ///
    /// # Errors
    /// [`NotADomain`] for the first `ours` entry that is not one.
    pub fn new<'a>(
        ours: impl IntoIterator<Item = &'a str>,
        keep: KeepList,
    ) -> Result<Self, NotADomain> {
        let domains = ours
            .into_iter()
            .map(|text| Domain::parse(text).map(|domain| (domain, Origin::Ours)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            domains,
            keep,
            seed: Seed::None,
        })
    }

    /// Folds a seed's domains in as [`Origin::Seed`] (an `Ours` label wins) and
    /// marks [`Seed::Loaded`].
    ///
    /// # Errors
    /// [`NotADomain`] for a seed line that is not one: the seed is normalised at
    /// build time, so this is a build defect not to paper over.
    pub fn with_seed<'a>(
        mut self,
        seed: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, NotADomain> {
        let mut domains = 0_usize;
        for text in seed {
            let domain = Domain::parse(text)?;
            domains += 1;
            self.domains.entry(domain).or_insert(Origin::Seed);
        }
        self.seed = Seed::Loaded { domains };
        Ok(self)
    }

    /// The `keep` list this was built with.
    #[must_use]
    pub fn keep(&self) -> &KeepList {
        &self.keep
    }

    /// Whether a seed was folded in, and how large.
    #[must_use]
    pub fn seed_status(&self) -> Seed {
        self.seed
    }

    /// Where mail from `sender` is churn, if it is: the covering entry's
    /// origin (the more deliberate one, when both cover it), or `None` when
    /// `keep` covers it or nothing does. An unaddressed sender is never
    /// covered.
    #[must_use]
    pub fn origin(&self, sender: &Sender) -> Option<Origin> {
        let Sender::Domain(domain) = sender else {
            return None;
        };
        if self.keep.covers(domain) {
            return None;
        }
        let mut found = None;
        for ancestor in domain.and_ancestors() {
            match self.domains.get(&ancestor) {
                Some(Origin::Ours) => return Some(Origin::Ours),
                Some(Origin::Seed) => found = Some(Origin::Seed),
                None => {}
            }
        }
        found
    }

    /// Whether mail from `sender` is churn at all.
    #[must_use]
    pub fn covers(&self, sender: &Sender) -> bool {
        self.origin(sender).is_some()
    }

    /// What an answer says of this list: the deployment's own entries and `keep` in full, and
    /// the seed by its size alone.
    #[must_use]
    pub fn summary(&self) -> ChurnSummary {
        ChurnSummary {
            ours: self
                .domains
                .iter()
                .filter(|(_, origin)| **origin == Origin::Ours)
                .map(|(domain, _)| domain.clone())
                .collect(),
            keep: self.keep.clone(),
            seed: self.seed,
        }
    }

    /// The listed domains, in order, with `keep` not subtracted (use
    /// [`Self::covers`] for the effective set).
    pub fn iter(&self) -> impl Iterator<Item = &Domain> {
        self.domains.keys()
    }
}

/// A churn list as an answer prints it: what this deployment declared, and how large a seed it
/// folded in, never the seed's domains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChurnSummary {
    /// `[mail.junk] churn`, in order.
    pub ours: Vec<Domain>,
    /// `[mail.junk] keep`.
    pub keep: KeepList,
    /// Whether a seed was folded in, and how many domains it held.
    pub seed: Seed,
}

/// Normalises one seed file into the [`Domain`]s it lists, in canonical form so
/// the runtime never refuses one. Comments, blanks and malformed lines are
/// dropped rather than failing the build: a public list will someday hold a bad
/// line, and `sources.txt`'s count makes the drop visible.
#[must_use]
pub fn normalize_seed_file(text: &str) -> BTreeSet<Domain> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| Domain::parse(line).ok())
        .collect()
}

/// A message's UID in the Junk folder: the handle `junk_show` takes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Uid(pub u32);

/// The most characters of a header's name, address, subject or date a digest keeps: spam
/// writes subjects of any length.
pub const HEADER_CHARS: usize = 200;

/// The most headers a digest lists in full per account, newest first.
pub const MAX_LISTED: usize = 50;

/// The most sender domains a digest counts by name per account, most first.
pub const MAX_DOMAINS: usize = 50;

/// One Junk message's headers, and the handle to open it by.
///
/// Built by [`JunkHeader::new`], which derives [`JunkHeader::sender`] from
/// the address, so the domain a digest counts is the address it shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JunkHeader {
    uid: Uid,
    from: String,
    #[serde(skip)]
    sender: Sender,
    subject: String,
    date: String,
}

impl JunkHeader {
    /// A header from its parts as the message printed them: the display name
    /// and address of `From` (either may be absent), the subject and the raw
    /// `Date`.
    #[must_use]
    pub fn new(
        uid: Uid,
        name: Option<&str>,
        address: Option<&str>,
        subject: &str,
        date: &str,
    ) -> Self {
        let address = address.map(str::trim).filter(|a| !a.is_empty());
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let from = match (name, address) {
            (Some(name), Some(address)) => format!("{name} <{address}>"),
            (None, Some(address)) => address.to_owned(),
            (Some(name), None) => name.to_owned(),
            (None, None) => String::new(),
        };
        Self {
            uid,
            from: super::shorten(&from, HEADER_CHARS).to_owned(),
            sender: address
                .and_then(Domain::of_address)
                .map_or(Sender::Unaddressed, Sender::Domain),
            subject: super::shorten(subject.trim(), HEADER_CHARS).to_owned(),
            date: super::shorten(date.trim(), HEADER_CHARS).to_owned(),
        }
    }

    /// The handle.
    #[must_use]
    pub fn uid(&self) -> Uid {
        self.uid
    }

    /// Where it came from.
    #[must_use]
    pub fn sender(&self) -> &Sender {
        &self.sender
    }
}

/// The days a sweep looks back over, ending today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Window {
    /// The first day read (IMAP `SINCE`, inclusive).
    pub since: NaiveDate,
    /// How many days.
    pub days: u32,
}

/// Why a window cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("days must be between 1 and {MAX_DAYS}, not {0}")]
pub struct BadWindow(pub usize);

/// A month, which is the sweep's own cadence.
pub const DEFAULT_DAYS: usize = 30;

/// A year. A longer look is a backfill, not a sweep.
pub const MAX_DAYS: u32 = 366;

impl Window {
    /// The `days` ending `today`.
    ///
    /// # Errors
    /// [`BadWindow`] outside `1..=MAX_DAYS`.
    pub fn ending(today: NaiveDate, days: usize) -> Result<Self, BadWindow> {
        let days = u32::try_from(days)
            .ok()
            .filter(|d| (1..=MAX_DAYS).contains(d))
            .ok_or(BadWindow(days))?;
        Ok(Self {
            since: today - chrono::Days::new(u64::from(days)),
            days,
        })
    }
}

/// One account's Junk, swept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JunkDigest {
    /// The account.
    pub account: String,
    /// What was read.
    pub window: Window,
    /// How many messages the window held.
    pub total: usize,
    /// The [`MAX_DOMAINS`] commonest sender domains, each with its count and where `churn`
    /// covers it from (`None` when nothing does), most first (ties by name).
    pub by_domain: Vec<(Sender, usize, Option<Origin>)>,
    /// How many sender domains are not in `by_domain`.
    pub domains_left_out: usize,
    /// The [`MAX_LISTED`] newest headers not from a churn domain, newest UID first.
    pub outside_churn: Vec<JunkHeader>,
    /// How many headers outside churn are not in `outside_churn`.
    pub listed_left_out: usize,
}

/// Counts every header by sender, labels each with `churn`'s [`Origin`] (or
/// none), and keeps in full the ones outside `churn`, each list cut at its bound and
/// counting what it left out.
#[must_use]
pub fn digest(
    account: &str,
    headers: Vec<JunkHeader>,
    churn: &ChurnList,
    window: Window,
) -> JunkDigest {
    let total = headers.len();
    let mut counts: BTreeMap<Sender, usize> = BTreeMap::new();
    for header in &headers {
        *counts.entry(header.sender.clone()).or_default() += 1;
    }
    let mut by_domain: Vec<(Sender, usize, Option<Origin>)> = counts
        .into_iter()
        .map(|(sender, count)| {
            let origin = churn.origin(&sender);
            (sender, count, origin)
        })
        .collect();
    by_domain.sort_by(|(a, m, _), (b, n, _)| n.cmp(m).then_with(|| a.cmp(b)));
    let mut outside_churn: Vec<JunkHeader> = headers
        .into_iter()
        .filter(|header| !churn.covers(&header.sender))
        .collect();
    outside_churn.sort_by_key(|header| std::cmp::Reverse(header.uid));
    let domains_left_out = by_domain.len().saturating_sub(MAX_DOMAINS);
    by_domain.truncate(MAX_DOMAINS);
    let listed_left_out = outside_churn.len().saturating_sub(MAX_LISTED);
    outside_churn.truncate(MAX_LISTED);
    JunkDigest {
        account: account.to_owned(),
        window,
        total,
        by_domain,
        domains_left_out,
        outside_churn,
        listed_left_out,
    }
}

/// One mailbox as `LIST` answered it: its name, and whether the server marked
/// it `\Junk` (RFC 6154 SPECIAL-USE).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// The server's name for it, unquoted.
    pub name: String,
    /// Carries the `\Junk` attribute.
    pub junk: bool,
}

/// The names a Junk folder goes by where the server marks none: personal
/// Outlook's, then Exchange's two.
pub const JUNK_NAMES: [&str; 3] = ["Junk", "Junk Email", "Junk E-mail"];

/// Which folder is Junk: the one marked `\Junk`, else the first whose name is
/// one of [`JUNK_NAMES`] (any case), else none.
#[must_use]
pub fn junk_folder(listed: &[Listed]) -> Option<&str> {
    listed
        .iter()
        .find(|folder| folder.junk)
        .or_else(|| {
            JUNK_NAMES.iter().find_map(|wanted| {
                listed
                    .iter()
                    .find(|folder| folder.name.eq_ignore_ascii_case(wanted))
            })
        })
        .map(|folder| folder.name.as_str())
}

/// One Junk message opened: its headers and its body as text, cut at
/// [`super::MAX_BODY_CHARS`] and saying so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JunkMessage {
    /// The account.
    pub account: String,
    /// Its headers.
    #[serde(flatten)]
    pub header: JunkHeader,
    /// The body, HTML converted to text.
    pub body: String,
    /// Whether the body was cut.
    pub truncated: bool,
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::cast::CAST;

    fn day(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
    }

    fn window() -> Window {
        Window::ending(day("2026-09-27"), 30).expect("a month")
    }

    fn header(uid: u32, address: &str) -> JunkHeader {
        JunkHeader::new(
            Uid(uid),
            None,
            Some(address),
            "a subject",
            "Sat, 26 Sep 2026",
        )
    }

    #[test]
    fn a_domain_is_lowered_and_loses_its_root_dot() {
        assert_eq!(
            Domain::parse("Offers.Example.NET.")
                .expect("a domain")
                .as_str(),
            "offers.example.net"
        );
        for bad in [
            "",
            "localhost",
            "a..b",
            "no spaces.test",
            "under_score.test",
        ] {
            assert!(Domain::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_subdomain_is_within_its_parent_and_a_lookalike_is_not() {
        let parent = Domain::parse("shop.test").expect("a domain");
        let child = Domain::parse("mail.shop.test").expect("a domain");
        let lookalike = Domain::parse("badshop.test").expect("a domain");
        assert!(parent.within(&parent));
        assert!(child.within(&parent));
        assert!(!lookalike.within(&parent));
        assert!(!parent.within(&child));
    }

    #[test]
    fn a_header_with_no_address_is_unaddressed_and_never_churn() {
        let bare = JunkHeader::new(Uid(1), Some("Prize Desk"), None, "You won", "");
        assert_eq!(bare.sender(), &Sender::Unaddressed);
        assert_eq!(bare.from, "Prize Desk");
        let churn = ChurnList::new(["example.test"], KeepList::default()).expect("a list");
        assert!(!churn.covers(bare.sender()));
    }

    #[test]
    fn a_header_shows_the_name_and_address_it_was_built_from() {
        let friend = JunkHeader::new(
            Uid(7),
            Some(CAST.friend.name),
            Some(CAST.friend.email),
            " RSVP ",
            "Fri, 25 Sep 2026 10:00:00 +0000",
        );
        assert_eq!(
            friend.from,
            format!("{} <{}>", CAST.friend.name, CAST.friend.email)
        );
        assert_eq!(friend.subject, "RSVP");
        assert_eq!(
            friend.sender(),
            &Sender::Domain(Domain::of_address(CAST.friend.email).expect("a domain"))
        );
    }

    #[test]
    fn a_churn_list_refuses_what_is_not_a_domain() {
        assert_eq!(
            ChurnList::new(["offers.example.net", "not a domain"], KeepList::default()),
            Err(NotADomain("not a domain".to_owned()))
        );
        assert_eq!(
            ChurnList::default(),
            ChurnList::new([], KeepList::default()).expect("empty")
        );
    }

    fn sender(address: &str) -> Sender {
        Sender::Domain(Domain::of_address(address).expect("a domain"))
    }

    /// `keep` beats the seed.
    #[test]
    fn keep_beats_seed() {
        let keep = KeepList::new(["offers.example.net"]).expect("a list");
        let churn = ChurnList::new([], keep)
            .expect("a list")
            .with_seed(["offers.example.net"])
            .expect("a seed");
        assert_eq!(churn.origin(&sender("deals@offers.example.net")), None);
        assert!(!churn.covers(&sender("deals@offers.example.net")));
    }

    /// `keep` beats `ours` too.
    #[test]
    fn keep_beats_ours_too() {
        let keep = KeepList::new(["offers.example.net"]).expect("a list");
        let churn = ChurnList::new(["offers.example.net"], keep).expect("a list");
        assert!(!churn.covers(&sender("deals@offers.example.net")));
    }

    /// `ours` and the seed union, each under its own [`Origin`]; `ours` wins
    /// where both list a domain.
    #[test]
    fn ours_and_seed_union() {
        let churn = ChurnList::new(["ours.test"], KeepList::default())
            .expect("a list")
            .with_seed(["seed.test", "ours.test"])
            .expect("a seed");
        assert_eq!(churn.origin(&sender("a@ours.test")), Some(Origin::Ours));
        assert_eq!(churn.origin(&sender("a@seed.test")), Some(Origin::Seed));
        assert_eq!(churn.origin(&sender("a@elsewhere.test")), None);
        assert_eq!(churn.seed_status(), Seed::Loaded { domains: 2 });
    }

    /// A subdomain at any depth is churn; a lookalike sharing a suffix is not.
    #[test]
    fn a_subdomain_of_a_listed_domain_is_churn() {
        let churn = ChurnList::new(["offers.example.net"], KeepList::default()).expect("a list");
        assert!(churn.covers(&sender("a@mail.deals.offers.example.net")));
        assert!(!churn.covers(&sender("a@badoffers.example.net")));
    }

    /// [`ChurnList::covers`] agrees with "has some origin".
    #[test]
    fn origin_labels_agree_with_covers() {
        let churn = ChurnList::new(["ours.test"], KeepList::default())
            .expect("a list")
            .with_seed(["seed.test"])
            .expect("a seed");
        for (address, expected) in [
            ("a@ours.test", Some(Origin::Ours)),
            ("a@seed.test", Some(Origin::Seed)),
            ("a@elsewhere.test", None),
        ] {
            let who = sender(address);
            assert_eq!(churn.origin(&who), expected, "{address}");
            assert_eq!(churn.covers(&who), expected.is_some(), "{address}");
        }
    }

    /// A seed of 200,000 domains is a lookup, not a scan.
    #[test]
    fn a_big_seed_is_fast_enough() {
        let synthetic: Vec<String> = (0..200_000)
            .map(|n| format!("seed-{n}.example.net"))
            .collect();
        let started = std::time::Instant::now();
        let churn = ChurnList::new(["ours.test"], KeepList::default())
            .expect("a list")
            .with_seed(synthetic.iter().map(String::as_str))
            .expect("a seed");
        assert_eq!(churn.seed_status(), Seed::Loaded { domains: 200_000 });
        for n in [0_usize, 99_999, 199_999] {
            assert_eq!(
                churn.origin(&sender(&format!("a@seed-{n}.example.net"))),
                Some(Origin::Seed)
            );
        }
        assert_eq!(churn.origin(&sender("a@not-in-the-seed.example.net")), None);
        assert_eq!(churn.origin(&sender("a@ours.test")), Some(Origin::Ours));
        let elapsed = started.elapsed();
        assert!(elapsed.as_secs() < 5, "took {elapsed:?}");
    }

    /// A sweep of thousands of headers with no churn answers a bounded digest: the newest
    /// [`MAX_LISTED`] headers and the [`MAX_DOMAINS`] commonest domains, counting the rest.
    #[test]
    fn a_large_sweep_is_bounded_and_counts_what_it_left_out() {
        let headers: Vec<JunkHeader> = (0..5000)
            .map(|n| {
                JunkHeader::new(
                    Uid(n),
                    Some(&"N".repeat(1000)),
                    Some(&format!("a@sender-{}.example.net", n % 300)),
                    &"S".repeat(10_000),
                    "Sat, 26 Sep 2026",
                )
            })
            .collect();
        let churn = ChurnList::new([], KeepList::default())
            .expect("a list")
            .with_seed(
                (0..200_000)
                    .map(|n| format!("seed-{n}.example.net"))
                    .collect::<Vec<_>>()
                    .iter()
                    .map(String::as_str),
            )
            .expect("a seed");
        let swept = digest(CAST.owner.email, headers, &churn, window());
        assert_eq!(swept.total, 5000);
        assert_eq!(swept.outside_churn.len(), MAX_LISTED);
        assert_eq!(swept.listed_left_out, 5000 - MAX_LISTED);
        assert_eq!(swept.outside_churn[0].uid(), Uid(4999), "newest first");
        assert_eq!(swept.by_domain.len(), MAX_DOMAINS);
        assert_eq!(swept.domains_left_out, 300 - MAX_DOMAINS);
        let printed = serde_json::json!({"churn": churn.summary(), "account": swept}).to_string();
        assert!(printed.len() < 64 * 1024, "{} bytes", printed.len());
        assert!(
            printed.contains(r#""seed":{"loaded":{"domains":200000}}"#),
            "the seed by its size"
        );
        assert!(
            !printed.contains("seed-0.example.net"),
            "never the seed's domains"
        );
    }

    #[test]
    fn a_window_is_one_to_a_year_of_days() {
        assert_eq!(window().since, day("2026-08-28"));
        assert_eq!(window().days, 30);
        assert_eq!(Window::ending(day("2026-09-27"), 0), Err(BadWindow(0)));
        assert_eq!(Window::ending(day("2026-09-27"), 367), Err(BadWindow(367)));
    }

    /// Churn and its subdomains are counted, not listed; everyone else is
    /// listed newest first.
    #[test]
    fn churn_is_counted_and_everything_else_is_listed() {
        let churn = ChurnList::new(["offers.example.net"], KeepList::default()).expect("a list");
        let headers = vec![
            header(1, "deals@offers.example.net"),
            header(2, CAST.friend.email),
            header(3, "news@mail.offers.example.net"),
            header(4, "deals@offers.example.net"),
            header(5, CAST.counsel.email),
            JunkHeader::new(Uid(6), None, None, "", ""),
        ];
        let swept = digest(CAST.owner.email, headers, &churn, window());
        assert_eq!(swept.total, 6);
        let uids: Vec<u32> = swept.outside_churn.iter().map(|h| h.uid().0).collect();
        assert_eq!(uids, vec![6, 5, 2]);
        let printed: Vec<(&str, usize, Option<Origin>)> = swept
            .by_domain
            .iter()
            .map(|(sender, count, origin)| (sender.as_str(), *count, *origin))
            .collect();
        assert_eq!(
            printed,
            vec![
                ("example.test", 2, None),
                ("offers.example.net", 2, Some(Origin::Ours)),
                ("mail.offers.example.net", 1, Some(Origin::Ours)),
                ("(no address)", 1, None),
            ]
        );
    }

    /// With no churn declared, every header is listed.
    #[test]
    fn an_empty_churn_list_lists_every_header() {
        let headers = vec![
            header(1, CAST.friend.email),
            header(2, "x@offers.example.net"),
        ];
        let swept = digest(CAST.owner.email, headers, &ChurnList::default(), window());
        assert_eq!(swept.outside_churn.len(), swept.total);
    }

    #[test]
    fn the_digest_serialises_senders_as_text_and_windows_as_dates() {
        let swept = digest(
            CAST.owner.email,
            vec![header(9, CAST.friend.email)],
            &ChurnList::default(),
            window(),
        );
        let json = serde_json::to_value(&swept).expect("serialises");
        assert_eq!(json["window"]["since"], "2026-08-28");
        assert_eq!(json["by_domain"][0][0], "example.test");
        assert_eq!(json["outside_churn"][0]["uid"], 9);
        assert_eq!(json["outside_churn"][0]["from"], CAST.friend.email);
    }

    #[test]
    fn junk_is_the_marked_folder_else_a_known_name_else_none() {
        let listed = |pairs: &[(&str, bool)]| -> Vec<Listed> {
            pairs
                .iter()
                .map(|(name, junk)| Listed {
                    name: (*name).to_owned(),
                    junk: *junk,
                })
                .collect()
        };
        let marked = listed(&[("Inbox", false), ("Junk", false), ("Spam Box", true)]);
        assert_eq!(junk_folder(&marked), Some("Spam Box"));
        let named = listed(&[("Inbox", false), ("junk email", false)]);
        assert_eq!(junk_folder(&named), Some("junk email"));
        let neither = listed(&[("Inbox", false), ("Sent", false)]);
        assert_eq!(junk_folder(&neither), None);
    }

    fn any_sender() -> impl Strategy<Value = Option<String>> {
        let domains = prop::sample::select(vec![
            "offers.example.net",
            "mail.offers.example.net",
            "promo.example.org",
            "example.test",
            "badoffers.example.net",
            "not a domain",
        ]);
        prop::option::of((prop::sample::select(vec!["a", "b", "news"]), domains))
            .prop_map(|pair| pair.map(|(local, domain)| format!("{local}@{domain}")))
    }

    fn any_churn() -> impl Strategy<Value = ChurnList> {
        prop::sample::subsequence(
            vec!["offers.example.net", "promo.example.org", "example.test"],
            0..=3,
        )
        .prop_map(|domains| ChurnList::new(domains, KeepList::default()).expect("domains"))
    }

    proptest! {
        /// The counts sum to the total, and the total is every header, when no domain was left
        /// out; the counts kept are each domain's whole count.
        #[test]
        fn the_counts_sum_to_the_total(
            senders in prop::collection::vec(any_sender(), 0..60),
            churn in any_churn(),
        ) {
            let count = senders.len();
            let headers: Vec<JunkHeader> = senders
                .iter()
                .enumerate()
                .map(|(at, sender)| JunkHeader::new(
                    Uid(u32::try_from(at).expect("small")), None, sender.as_deref(), "", "",
                ))
                .collect();
            let swept = digest("x@example.test", headers, &churn, window());
            prop_assert_eq!(swept.total, count);
            let summed = swept.by_domain.iter().map(|(_, n, _)| n).sum::<usize>();
            prop_assert!(swept.by_domain.len() <= MAX_DOMAINS);
            if swept.domains_left_out == 0 {
                prop_assert_eq!(summed, count);
            } else {
                prop_assert!(summed < count);
            }
        }

        /// A churn domain is never listed, and whatever outside churn is not listed is counted.
        #[test]
        fn a_churn_domain_is_never_listed(
            senders in prop::collection::vec(any_sender(), 0..60),
            churn in any_churn(),
        ) {
            let headers: Vec<JunkHeader> = senders
                .iter()
                .enumerate()
                .map(|(at, sender)| JunkHeader::new(
                    Uid(u32::try_from(at).expect("small")), None, sender.as_deref(), "", "",
                ))
                .collect();
            let kept = headers.iter().filter(|h| !churn.covers(h.sender())).count();
            let swept = digest("x@example.test", headers, &churn, window());
            for listed in &swept.outside_churn {
                if let Sender::Domain(domain) = listed.sender() {
                    prop_assert!(churn.iter().all(|c| !domain.within(c)));
                }
            }
            prop_assert!(swept.outside_churn.len() <= MAX_LISTED);
            prop_assert_eq!(swept.outside_churn.len() + swept.listed_left_out, kept);
        }
    }

    /// The normaliser strips comments and blanks, lowers case, deduplicates,
    /// and drops a malformed line rather than failing, on the shapes the real
    /// seed files have.
    #[test]
    fn the_seed_normaliser_handles_comments_blanks_case_and_duplicates() {
        let fixture = "\
            # Title: Example Block List\n\
            # Homepage: https://example.test\n\
            #\n\
            \n\
            Offers.Example.NET\n\
            offers.example.net\n\
            \n\
            mail.offers.example.net\n\
            not-a-domain\n\
        ";
        let normalized = normalize_seed_file(fixture);
        let printed: Vec<&str> = normalized.iter().map(Domain::as_str).collect();
        assert_eq!(
            printed,
            vec!["mail.offers.example.net", "offers.example.net"]
        );
    }
}

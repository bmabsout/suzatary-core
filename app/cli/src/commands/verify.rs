//! `suzatary verify` — the stores' fsck.
//!
//! Every object rehashed, every parent present, one line per problem and a non-zero exit
//! when there is one: `scripts/safe-push.sh` aborts on it before committing, since a bad
//! object in a Merkle log is history that must not reach the remote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::bail;
use suzatary_core::chains::ChainName;
use suzatary_core::store::{Area, Database, Kind};

use super::{proposals_of, proposing_of, store_of};
use crate::commands::serve::interpreters::{file_store, upload_store};
use crate::instance::Instance;

/// `suzatary verify` — fsck every store the data push stages.
///
/// The database's own fsck, [`Database::verify`], over every prodrome it holds, and beside it
/// the request records, which the database does not hold. A replica is read as the agent's run
/// opens it, its chain's objects taken in first, since a key the chain registered since the
/// last run proves what the replica holds. The plugins' stores are the cache tier, never
/// pushed and rebuilt by syncing, so a damaged one is no reason to hold the data back.
pub fn run(path: &Path) -> anyhow::Result<()> {
    let instance = Instance::open(path)?;
    let mut findings = Vec::new();
    let mut areas = Vec::new();
    for chain in &instance.chains {
        // Opening the replica as the agent's run does takes its chain's objects in on disk,
        // so the replica read after it is the one the run would read.
        if let Err(why) = proposing_of(chain) {
            findings.push((
                format!("{} proposals", chain.policy.name),
                vec![format!("{why:#}")],
            ));
        }
        let proposals = proposals_of(chain);
        areas.push((chain, store_of(chain), proposals));
    }
    let inbox = instance.inbox_store();
    let conversations = instance.conversation_stores();
    let memory = instance.memory_store();
    let memory_texts = suzatary_io::texts::Texts::open(instance.secretary.memory_texts());
    let digests = instance.digest_store();
    let digest_texts = suzatary_io::texts::Texts::open(instance.secretary.digest_texts());
    let decisions = suzatary_io::objects::Objects::open(instance.decisions());
    let topics = instance.secretary.topics.open();
    let triage = suzatary_io::state::State::open(instance.mail.state.clone());
    let triage_local = suzatary_io::state::Local::open(instance.local.mail());
    let devices = suzatary_io::devices::Devices::open(instance.paths.devices.clone());
    let uploads = upload_store(&instance);
    let kept = file_store(&instance);
    let chains: BTreeMap<ChainName, Area<'_>> = areas
        .iter()
        .map(|(chain, events, proposals)| (chain.policy.name.clone(), Area { events, proposals }))
        .collect();
    let database = Database {
        chains,
        inbox: &inbox,
        conversations: &conversations.lines,
        line_keys: &conversations.keys,
        memory: &memory,
        memory_texts: &memory_texts,
        digests: &digests,
        digest_texts: &digest_texts,
        decisions: &decisions,
        topics: &topics.memberships,
        topic_aliases: &topics.aliases,
        plugins: BTreeMap::new(),
        triage: &triage,
        triage_local: &triage_local,
        devices: &devices,
        uploads: &uploads,
        kept: &kept,
    };
    let requests =
        suzatary_io::requests::Requests::open(instance.requests(), instance.paths.answers.clone());
    findings.extend(database.verify());
    findings.push(("requests".to_owned(), requests.verify()));

    // Where each store lives, by the name its findings carry, so an OK line says which
    // directory was read.
    let roots: BTreeMap<String, PathBuf> = Kind::ALL
        .iter()
        .flat_map(|kind| instance.directories(*kind))
        .collect();

    let mut total = 0;
    for (what, problems) in &findings {
        total += problems.len();
        if problems.is_empty() {
            match roots.get(what) {
                Some(root) => println!("{what} OK — {}", root.display()),
                None => println!("{what} OK"),
            }
        }
        for problem in problems {
            eprintln!("{what}: {problem}");
        }
    }
    if total == 0 {
        return Ok(());
    }
    bail!("{total} problem(s) across the stores the data push stages")
}

//! The agent's first answer leaves a fifth of its cap free, so `OPERATOR.md` can grow without a
//! silent cut. Weighed with this box's persona and categories where there is an instance file,
//! and otherwise with a stand-in of the same shape; skipped where the checkout's manual is not
//! present (a build sandbox is handed `app/` alone).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use prodrome_core::policy::Untrusted;
use suzatary_core::agent::{CONTEXT_CAP, CONTEXT_HEADROOM, context};
use suzatary_core::chains::{ChainName, ChainPolicy, Chains};
use suzatary_core::deployment::Deployment;
use suzatary_core::persona::Persona;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The scale's seven anchors and a dozen categories, about what a deployment declares.
fn stand_in() -> (Persona, Deployment) {
    let persona = Persona {
        name: "Ada".to_owned(),
        city: "London".to_owned(),
        zone: suzatary_core::zone::Zone::UTC,
        prose: suzatary_core::persona::Prose {
            news_filter: Vec::new(),
            email_voice: Vec::new(),
            anchors: [
                "10%: an emergency where life is at stake this hour",
                "20%: an irreversible legal or financial blow that lands tomorrow",
                "35-45%: a real consequence within weeks, such as a government filing",
                "50-65%: work that matters this month",
                "70-85%: errands, forms and habits",
                "90%+: parked, someday",
                "a deadline bounds when, never the stakes",
            ]
            .map(str::to_owned)
            .to_vec(),
        },
    };
    let deployment = Deployment {
        viewers: BTreeSet::new(),
        actors: BTreeMap::new(),
        chains: Chains::of(vec![ChainPolicy {
            name: ChainName::new("life").expect("a slug"),
            untrusted: Untrusted::of([]),
            roster: BTreeSet::new(),
            root: suzatary_core::cast::keys::root(),
        }])
        .expect("one chain"),
        categories: (0..12u32)
            .map(|n| (format!("category-{n:02}"), n))
            .collect(),
        app_url: "https://example.invalid/".to_owned(),
        coding: None,
        zone: suzatary_core::zone::Zone::UTC,
    };
    (persona, deployment)
}

#[test]
fn the_agents_context_leaves_a_fifth_of_its_cap_for_the_manual_to_grow() {
    let repo = repo_root();
    if !repo.join("OPERATOR.md").is_file() {
        eprintln!("skipped: no OPERATOR.md beside app/ (a sandbox holds app/ alone)");
        return;
    }
    let (persona, deployment) = crate::this_box::instance().map_or_else(stand_in, |instance| {
        (instance.persona.clone(), instance.deployment.clone())
    });
    let handbook = suzatary_io::manuals::Handbook::new(&repo, persona);
    let wire = serde_json::to_string(&context(&handbook, &deployment)).expect("it serialises");
    let budget = CONTEXT_CAP - CONTEXT_HEADROOM;
    assert!(
        wire.len() <= budget,
        "context answers {} bytes, past the {budget} that leave {CONTEXT_HEADROOM} of the \
         {CONTEXT_CAP}-byte cap free; carry less inline or tighten OPERATOR.md",
        wire.len()
    );
    assert!(!wire.contains("cut here"), "the manual was cut");
    println!(
        "context: {} bytes of {CONTEXT_CAP}, {} free ({}%)",
        wire.len(),
        CONTEXT_CAP - wire.len(),
        (CONTEXT_CAP - wire.len()) * 100 / CONTEXT_CAP
    );
}

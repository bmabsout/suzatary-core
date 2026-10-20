//! The laws a sync is held to beyond its schema's own (the plugins plan, §5), checked over a run
//! and its replays before anything it reported is learned.

use std::fmt;

use prodrome_core::declared::{Declaration, Declared};

use super::{ItemRefusal, Knowledge, Manifest, Synced, admit};

/// A law of a sync, in the order they are asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Law {
    /// Every item it emits parses into its declared schema, prints back the same, and says the
    /// plugin itself wrote it.
    Items,
    /// Run again from the same cursor over the same answers and the same instant, it reports the
    /// same items and the same cursor: a sync is a function of what it was given.
    Deterministic,
    /// Run from its own new cursor over the same answers, it teaches the store nothing the first
    /// run did not: syncing twice is syncing once.
    Idempotent,
}

impl Law {
    /// Its name, as a refusal says it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Law::Items => "every item is an event of its schema",
            Law::Deterministic => "a sync is a function of what it was given",
            Law::Idempotent => "syncing twice is syncing once",
        }
    }
}

impl fmt::Display for Law {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A run that broke a law: which, and what showed it. Nothing it reported is learned.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the law that {law} does not hold: {detail}")]
pub struct Broken {
    /// The law.
    pub law: Law,
    /// What showed it, naming no more of an item than a refusal does.
    pub detail: String,
}

/// What one sync and its replays answered, each a sync's own answer or why it gave none.
#[derive(Debug, Clone, Copy)]
pub struct Answers<'r> {
    /// The sync, live.
    pub run: &'r Synced,
    /// The same cursor again, answered as the live run was.
    pub again: Result<&'r Synced, &'r str>,
    /// From the live run's cursor, answered the same.
    pub onward: Result<&'r Synced, &'r str>,
}

fn admitted(
    schema: &Declaration,
    manifest: &Manifest,
    synced: &Synced,
) -> Result<Vec<Declared>, ItemRefusal> {
    synced
        .items
        .iter()
        .map(|item| admit(schema, manifest, &item.schema, &item.bytes))
        .collect()
}

impl Answers<'_> {
    /// The live run's events, once every law holds of these answers over a store that reads
    /// `known`.
    ///
    /// # Errors
    /// The first [`Broken`] law, in [`Law`]'s order.
    pub fn hold(
        &self,
        schema: &Declaration,
        manifest: &Manifest,
        known: &Knowledge,
    ) -> Result<Vec<Declared>, Broken> {
        let broken = |law, detail: String| Broken { law, detail };
        let events = admitted(schema, manifest, self.run)
            .map_err(|refused| broken(Law::Items, refused.to_string()))?;
        match self.again {
            Ok(again) if again == self.run => {}
            Ok(again) if again.items != self.run.items => {
                return Err(broken(
                    Law::Deterministic,
                    format!(
                        "it reported {} items, then {} from the same cursor",
                        self.run.items.len(),
                        again.items.len()
                    ),
                ));
            }
            Ok(_) => {
                return Err(broken(
                    Law::Deterministic,
                    "it wrote another cursor from the same one".to_owned(),
                ));
            }
            Err(failed) => {
                return Err(broken(
                    Law::Deterministic,
                    format!("from the same cursor it failed: {failed}"),
                ));
            }
        }
        let onward = self.onward.map_err(|failed| {
            broken(
                Law::Idempotent,
                format!("from its own cursor it failed: {failed}"),
            )
        })?;
        let mut after = known.clone();
        for event in &events {
            after.settle(event);
        }
        let taught = admitted(schema, manifest, onward)
            .map_err(|refused| broken(Law::Items, refused.to_string()))?
            .iter()
            .filter(|event| !after.knows(event))
            .count();
        if taught > 0 {
            return Err(broken(
                Law::Idempotent,
                format!("from its own cursor it reported {taught} items the store did not hold"),
            ));
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use prodrome_core::literal::print_str;

    use super::*;
    use crate::plugin::Item;

    const SCHEMA: &str = "Schema(key='door', events=(Event(name='Seen', fields=(Field(name='door', \
        type=Text()), Field(name='at', type=Instant()), Field(name='actor', type=Text()), \
        Field(name='state', type=Enum(alternatives=('open', 'shut'))))),), \
        registers=(Register(name='state', order=Discrete(), inflationary=False, \
        writes=(Write(event='Seen', field='state'),)),))";

    fn manifest() -> Manifest {
        toml::from_str(
            r#"
            name = "probe"
            component = "probe.wasm"
            schema = "probe.schema"
            world = "source@0.3.0"
            "#,
        )
        .expect("a manifest")
    }

    fn synced(schema: &Declaration, seen: &[(&str, u32, &str)], cursor: &str) -> Synced {
        Synced {
            items: seen
                .iter()
                .map(|(door, minute, state)| Item {
                    schema: schema.name().as_str().to_owned(),
                    bytes: format!(
                        "Seen(door={}, at=datetime(2026, 10, 5, 10, {minute}, 0), actor='probe', state={})",
                        print_str(door),
                        print_str(state)
                    )
                    .into_bytes(),
                })
                .collect(),
            cursor: cursor.as_bytes().to_vec(),
        }
    }

    fn held(answers: Answers<'_>) -> Result<usize, Law> {
        let schema = Declaration::admit(SCHEMA).expect("lawful");
        answers
            .hold(&schema, &manifest(), &Knowledge::default())
            .map(|events| events.len())
            .map_err(|broken| broken.law)
    }

    #[test]
    fn a_lawful_sync_is_learned_and_each_broken_law_is_named() {
        let schema = Declaration::admit(SCHEMA).expect("lawful");
        let run = synced(&schema, &[("front", 0, "open"), ("back", 0, "shut")], "1");
        let nothing = synced(&schema, &[], "1");
        let again_known = synced(&schema, &[("front", 1, "open")], "1");
        let lawful = |onward| Answers {
            run: &run,
            again: Ok(&run),
            onward,
        };
        assert_eq!(held(lawful(Ok(&nothing))), Ok(2));
        assert_eq!(
            held(lawful(Ok(&again_known))),
            Ok(2),
            "reporting what is known again teaches nothing"
        );

        let taught = synced(&schema, &[("front", 1, "shut")], "1");
        assert_eq!(held(lawful(Ok(&taught))), Err(Law::Idempotent));
        assert_eq!(held(lawful(Err("trapped"))), Err(Law::Idempotent));

        let other = synced(&schema, &[("front", 0, "open")], "1");
        let moved = synced(&schema, &[("front", 0, "open"), ("back", 0, "shut")], "2");
        for again in [Ok(&other), Ok(&moved), Err("trapped")] {
            assert_eq!(
                held(Answers {
                    run: &run,
                    again,
                    onward: Ok(&nothing),
                }),
                Err(Law::Deterministic)
            );
        }

        let mut bad = run.clone();
        bad.items[1].bytes = b"Seen(door='back')".to_vec();
        assert_eq!(
            held(Answers {
                run: &bad,
                again: Ok(&bad),
                onward: Ok(&nothing),
            }),
            Err(Law::Items)
        );
    }
}

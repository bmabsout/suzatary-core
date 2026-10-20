//! The version as a reading of the project chain (`suzatary_core::release`): a function of the
//! set of objects, monotone in the objects and in the instant, moved to a new major by a
//! release's completion; and the step from one release to the next always climbs.

use prodrome_core::event::{Actor, TodoId};
use prodrome_core::event::{Hash, mk_cancelled, mk_completed, mk_reopened, mk_spec_revised};
use prodrome_core::fpl::{self, mk_conj, mk_flat, mk_ref};
use prodrome_core::literal::Datetime;
use prodrome_core::policy::Policy;
use prodrome_core::policy::Untrusted;
use prodrome_core::term::Term;
use proptest::prelude::*;
use std::collections::BTreeMap;
use suzatary_core::record::TodoEvent;
use suzatary_core::release::{
    Completed, Completes, Counted, Lot, Named, Place, Plan, PlanError, Reading, Release, Released,
    Standing, Version,
};

const OWNER: &str = "owner";
const AGENT: &str = "triage";

fn day(n: i64) -> Datetime {
    let start = Datetime::new(2026, 9, 1, 12, 0, 0, 0).expect("a real instant");
    fpl::datetime_of(fpl::instant_of(start) + chrono::Duration::days(n)).expect("in range")
}

fn untrusted() -> Untrusted {
    Untrusted::of([Actor::new(AGENT).expect("an actor")])
}

fn todo(id: &str) -> TodoId {
    TodoId::new(id).expect("a todo id")
}

/// 2.0, 2.1 and 3.0, on the chain `suzatary`, from 1.
fn plan() -> Plan {
    Plan::new(
        "suzatary",
        1,
        vec![
            Release {
                todo: todo("two"),
                major: 2,
            },
            Release {
                todo: todo("twoone"),
                major: 2,
            },
            Release {
                todo: todo("three"),
                major: 3,
            },
        ],
    )
    .expect("a plan")
}

const RELEASES: [&str; 3] = ["two", "twoone", "three"];
const ITEMS: [&str; 6] = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];

fn composes(items: &[&str]) -> prodrome_core::term::Term {
    let mut terms = vec![mk_flat(0.6).expect("a value")];
    terms.extend(
        items
            .iter()
            .map(|i| mk_ref((*i).to_owned()).expect("a ref")),
    );
    mk_conj(terms, -4.0).expect("a conjunction")
}

/// One object of a random history.
#[derive(Debug, Clone)]
enum Op {
    Complete(usize, bool),
    Cancel(usize, bool),
    Reopen(usize),
    Revise(usize, Vec<usize>, bool),
}

/// Todos 0..6 are items, 6..9 the releases.
fn id(n: usize) -> &'static str {
    if n < ITEMS.len() {
        ITEMS[n]
    } else {
        RELEASES[n - ITEMS.len()]
    }
}

fn event(op: &Op, at: Datetime) -> TodoEvent {
    let actor = |trusted: bool| if trusted { OWNER } else { AGENT };
    match op {
        Op::Complete(n, trusted) => mk_completed(id(*n), at, actor(*trusted), ""),
        Op::Cancel(n, trusted) => mk_cancelled(id(*n), at, actor(*trusted), ""),
        Op::Reopen(n) => mk_reopened(id(*n), at, OWNER, ""),
        Op::Revise(r, items, trusted) => mk_spec_revised(
            RELEASES[*r],
            at,
            actor(*trusted),
            composes(&items.iter().map(|i| ITEMS[*i]).collect::<Vec<_>>()),
            "",
        ),
    }
    .expect("an event")
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..9usize, prop::bool::weighted(0.8)).prop_map(|(n, t)| Op::Complete(n, t)),
        (0..9usize, prop::bool::weighted(0.8)).prop_map(|(n, t)| Op::Cancel(n, t)),
        (0..9usize).prop_map(Op::Reopen),
        (
            0..3usize,
            prop::collection::vec(0..6usize, 0..6),
            prop::bool::weighted(0.8)
        )
            .prop_map(|(r, items, t)| Op::Revise(r, items, t)),
    ]
}

/// A history: each object named by its place, stamped on a day of the month.
fn history() -> impl Strategy<Value = Vec<(Hash, TodoEvent)>> {
    prop::collection::vec((op(), 0..30i64), 0..40).prop_map(|ops| {
        ops.iter()
            .enumerate()
            .map(|(n, (op, d))| {
                (
                    Hash::new(format!("{n:064x}")).expect("a name"),
                    event(op, day(*d)),
                )
            })
            .collect()
    })
}

/// The events of `objects` that bind under the test's policy.
fn bound(objects: &[(Hash, TodoEvent)]) -> Vec<&TodoEvent> {
    let policy = untrusted();
    objects
        .iter()
        .filter(|(name, event)| policy.standing(name, event).binds())
        .map(|(_, event)| event)
        .collect()
}

/// Each release's spec in force at `at`: every revision stamped at the latest instant by
/// then, several being a conflict, so the helper is a function of the set as the fold is.
fn current(events: &[&TodoEvent], at: Datetime) -> BTreeMap<TodoId, Vec<Term>> {
    let mut specs: BTreeMap<TodoId, (Datetime, Vec<Term>)> = BTreeMap::new();
    for event in events {
        if let prodrome_core::event::TodoEvent::SpecRevised(e) = event
            && e.at <= at
        {
            let held = specs.entry(e.todo.clone()).or_insert((e.at, Vec::new()));
            if held.0 < e.at {
                *held = (e.at, Vec::new());
            }
            if held.0 == e.at && !held.1.contains(&e.spec) {
                held.1.push(e.spec.clone());
            }
        }
    }
    specs
        .into_iter()
        .map(|(todo, (_, mut candidates))| {
            candidates.sort_by_key(prodrome_core::fpl::print_term);
            (todo, candidates)
        })
        .collect()
}

fn read(objects: &[(Hash, TodoEvent)], at: Datetime) -> Reading {
    let events = bound(objects);
    Reading::of(&plan(), events.iter().copied(), at, &current(&events, at))
}

proptest! {
    /// A function of the SET of objects: the order they are read in moves nothing.
    #[test]
    fn the_reading_is_a_function_of_the_set(objects in history(), seed in any::<u64>()) {
        let mut shuffled = objects.clone();
        // A deterministic permutation from the seed.
        shuffled.sort_by_key(|(h, _)| {
            let mut x = seed ^ u64::from_str_radix(&h.as_str()[48..], 16).unwrap_or(0);
            x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            x
        });
        prop_assert_eq!(read(&objects, day(40)), read(&shuffled, day(40)));
    }

    /// More objects never read a lower version: the law that keeps every tag above the last.
    #[test]
    fn the_version_is_monotone_in_the_objects(objects in history(), cut in 0..40usize) {
        let some = &objects[..cut.min(objects.len())];
        prop_assert!(read(some, day(40)).counted <= read(&objects, day(40)).counted);
    }

    /// A later instant never reads a lower version.
    #[test]
    fn the_version_is_monotone_in_time(objects in history(), a in 0..32i64, b in 0..32i64) {
        let (early, late) = (a.min(b), a.max(b));
        prop_assert!(read(&objects, day(early)).counted <= read(&objects, day(late)).counted);
    }

    /// Completing a release moves MAJOR to at least its major.
    #[test]
    fn completing_a_release_moves_the_major(objects in history(), r in 0..3usize) {
        let before = read(&objects, day(40)).counted;
        let mut after = objects.clone();
        after.push((Hash::new(format!("{:064x}", 999)).expect("a name"),
            mk_completed(RELEASES[r], day(31), OWNER, "").expect("an event")));
        let read_after = read(&after, day(40)).counted;
        let major = plan().releases()[r].major;
        prop_assert!(read_after.major >= major);
        prop_assert!(read_after >= before);
        if major > before.major {
            prop_assert!(read_after.major > before.major);
        }
    }

    /// What the agent writes binds nothing until the owner confirms it, so it moves no version.
    #[test]
    fn untrusted_objects_move_nothing(objects in history()) {
        let trusted: Vec<_> = objects.iter().filter(|(_, e)| e.actor().as_str() != AGENT).cloned().collect();
        prop_assert_eq!(read(&objects, day(40)), read(&trusted, day(40)));
    }

    /// The next release is above the last whenever the chain allows one, and refused only when
    /// the chain reads below it.
    #[test]
    fn the_next_version_always_climbs(
        major in 0..5u32, minor in 0..20u32, patch in 0..9u32, rmajor in 0..5u32, rminor in 0..20u32,
    ) {
        let last = Version { major, minor, patch };
        let read = Counted { major: rmajor, minor: rminor };
        match last.next(read) {
            Ok(next) => {
                prop_assert!(next > last);
                prop_assert_eq!(next.counted(), read);
            }
            Err(behind) => prop_assert!(behind.read < last.counted()),
        }
    }

    /// A version prints and parses as itself.
    #[test]
    fn a_version_round_trips(major in any::<u32>(), minor in any::<u32>(), patch in any::<u32>()) {
        let v = Version { major, minor, patch };
        prop_assert_eq!(v.to_string().parse::<Version>(), Ok(v));
    }
}

/// The items `suzatarytwo` composed on 2026-09-28.
const TWO: [&str; 15] = [
    "prodromemain",
    "changeidentity",
    "stagingoverlay",
    "prodromesets",
    "durablestore",
    "scheduletype",
    "typedverbs",
    "wireonce",
    "teststrust",
    "portsuites",
    "instancesplit",
    "onboarding",
    "decisionitems",
    "cleanuppasses",
    "finalizepublic",
];

/// Its items still open on 2026-10-12.
const OPEN: [&str; 4] = [
    "portsuites",
    "onboarding",
    "cleanuppasses",
    "finalizepublic",
];

/// `objects`, each named by its place, read for the project's plan.
fn project(objects: &[TodoEvent]) -> Reading {
    let plan = Plan::new(
        "suzatary",
        1,
        vec![
            Release {
                todo: todo("suzatarytwo"),
                major: 2,
            },
            Release {
                todo: todo("suzatarytwoone"),
                major: 2,
            },
        ],
    )
    .expect("a plan");
    let named: Vec<(Hash, TodoEvent)> = objects
        .iter()
        .enumerate()
        .map(|(n, e)| (Hash::new(format!("{n:064x}")).expect("a name"), e.clone()))
        .collect();
    let events = bound(&named);
    Reading::of(
        &plan,
        events.iter().copied(),
        day(60),
        &current(&events, day(60)),
    )
}

/// The project chain as the operator described it on 2026-10-12: `suzatarytwo` composing
/// fifteen items from 2026-09-28, ten completed, `instancesplit` cancelled, four open.
#[test]
fn the_project_chain_reads_one_ten() {
    let items = TWO;
    let reading = project;
    let mut objects = vec![
        mk_spec_revised("suzatarytwo", day(27), OWNER, composes(&items), "").expect("an event"),
    ];
    for item in items {
        if item == "instancesplit" {
            objects.push(mk_cancelled(item, day(32), OWNER, "superseded").expect("an event"));
        } else if !OPEN.contains(&item) {
            objects.push(mk_completed(item, day(33), OWNER, "").expect("an event"));
        }
    }

    let now = reading(&objects);
    assert_eq!(
        now.counted,
        Counted {
            major: 1,
            minor: 10
        }
    );
    let two = &now.releases[0];
    assert_eq!(two.place, Place::Next);
    assert_eq!(
        two.items
            .iter()
            .find(|i| i.todo.as_str() == "instancesplit")
            .map(|i| i.standing(two.place)),
        Some(Standing::Cancelled)
    );

    // The operator narrows 2.0: onboarding moves to a later release. A dropped item stays an
    // item, so nothing counted can stop counting.
    let narrowed: Vec<&str> = items
        .iter()
        .copied()
        .filter(|i| *i != "onboarding")
        .collect();
    objects.push(
        mk_spec_revised(
            "suzatarytwo",
            day(41),
            OWNER,
            composes(&narrowed),
            "onboarding is 2.2",
        )
        .expect("an event"),
    );
    assert_eq!(
        reading(&objects).counted,
        Counted {
            major: 1,
            minor: 10
        }
    );

    let narrowed_two = &reading(&objects).releases[0];
    let onboarding = narrowed_two
        .items
        .iter()
        .find(|i| i.todo.as_str() == "onboarding")
        .expect("still an item");
    assert_eq!(onboarding.named, Named::Dropped);

    // A reopened item stays counted: what shipped stays shipped.
    objects.push(mk_reopened("wireonce", day(42), OWNER, "").expect("an event"));
    assert_eq!(
        reading(&objects).counted,
        Counted {
            major: 1,
            minor: 10
        }
    );

    // A train closes portsuites: 1.11.
    objects.push(mk_completed("portsuites", day(43), OWNER, "PR #160").expect("an event"));
    assert_eq!(
        reading(&objects).counted,
        Counted {
            major: 1,
            minor: 11
        }
    );

    // The release completes: 2.0, and 2.1 is next.
    objects.push(mk_completed("suzatarytwo", day(44), OWNER, "").expect("an event"));
    let shipped = reading(&objects);
    assert_eq!(shipped.counted, Counted { major: 2, minor: 0 });
    assert_eq!(
        (shipped.releases[0].place, shipped.releases[1].place),
        (Place::Earlier, Place::Next)
    );
    assert_eq!(
        Version {
            major: 1,
            minor: 11,
            patch: 3
        }
        .next(shipped.counted),
        Ok(Version {
            major: 2,
            minor: 0,
            patch: 0
        })
    );
}

/// The declaration this build carries reads, and its releases are 2.0, 2.1 and 2.2's todos.
#[test]
fn the_declared_plan_reads() {
    let plan = Plan::declared().expect("app/core/releases.toml reads");
    assert_eq!(plan.chain(), "suzatary");
    assert_eq!(
        plan.releases().first().map(|r| r.todo.as_str()),
        Some("suzatarytwo")
    );
}

/// A plan whose majors go down, or stay at the base, or that names a release twice, is not one.
#[test]
fn a_plan_refuses_what_would_let_the_version_fall() {
    let r = |id: &str, major| Release {
        todo: todo(id),
        major,
    };
    assert!(matches!(
        Plan::new("c", 1, vec![r("a", 3), r("b", 2)]),
        Err(PlanError::Backwards { .. })
    ));
    assert!(matches!(
        Plan::new("c", 1, vec![r("a", 1)]),
        Err(PlanError::Backwards { .. })
    ));
    assert!(matches!(
        Plan::new("c", 1, vec![r("a", 2), r("a", 3)]),
        Err(PlanError::Twice(_))
    ));
}

/// The file a release writes reads back as itself, each todo once and each title one line.
#[test]
fn a_release_prints_and_parses_as_itself() {
    let completes = Completes::new([
        Completed {
            todo: todo("calendarlinks"),
            title: "calendar \"links\"\nand more".to_owned(),
        },
        Completed {
            todo: todo("calendarlinks"),
            title: "again".to_owned(),
        },
        Completed {
            todo: todo("portsuites"),
            title: "ports as traits".to_owned(),
        },
    ]);
    assert_eq!(completes.todos().len(), 2);
    assert_eq!(completes.todos()[0].title, "calendar \"links\"");
    let version = Version {
        major: 2,
        minor: 3,
        patch: 1,
    };
    for todos in [vec![todo("calendarlinks"), todo("portsuites")], Vec::new()] {
        let released = Released::new(version, todos);
        assert_eq!(Released::parse(&released.print_over("")), Ok(released));
    }
    // The file is public: it names ids, never a title.
    let cut = suzatary_core::release::Cut {
        version,
        completes: completes.clone(),
        folds: None,
    };
    let printed = cut.released().print_over("");
    assert!(
        printed.contains("completes = [\"calendarlinks\", \"portsuites\"]")
            && !printed.contains("calendar \"links")
            && !printed.contains("ports as traits"),
        "{printed}"
    );
    assert_eq!(
        completes.changelog("suzatary"),
        vec![
            "- Completes `todo:suzatary/calendarlinks`: calendar \"links\"".to_owned(),
            "- Completes `todo:suzatary/portsuites`: ports as traits".to_owned(),
        ]
    );
}

/// What semver would not spell is not a version.
#[test]
fn a_version_is_three_plain_numbers() {
    for bad in ["1.2", "1.2.3.4", "01.2.3", "1.-2.3", "v1.2.3", "1.2.x", ""] {
        assert!(bad.parse::<Version>().is_err(), "{bad:?} parsed");
    }
}

/// `GET /api/releases` over a set holding the plan's chain: each release priced as its row,
/// titled by its action line, its items with what counts; and nothing where the set holds no
/// such chain.
#[test]
fn the_release_route_reads_the_plans_chain() {
    use suzatary_core::store::{InMemory, Prodrome};
    let store = InMemory::<TodoEvent>::default();
    let authored = |id: &str, body: &str, spec| {
        suzatary_core::record::mk_authored(
            id,
            day(0),
            OWNER,
            "todo",
            day(0),
            suzatary_core::record::MarkupSource::escaped(body),
            spec,
            vec!["a fixture".to_owned()],
            "",
            "",
            suzatary_core::record::MarkupSource::empty(),
            None,
            Vec::new(),
            Vec::new(),
            "",
        )
        .expect("a record")
    };
    for event in [
        authored("two", "Suzatary 2.0", composes(&["alpha", "bravo"])),
        authored("alpha", "the alpha item", mk_flat(0.7).expect("a value")),
        authored("bravo", "the bravo item", mk_flat(0.7).expect("a value")),
        mk_completed("alpha", day(1), OWNER, "").expect("an event"),
    ] {
        store.append(event).expect("appends");
    }
    let policy = suzatary_core::chains::ChainPolicy {
        name: suzatary_core::chains::ChainName::new("suzatary").expect("a slug"),
        untrusted: untrusted(),
        roster: [Actor::new(OWNER).expect("an actor")].into_iter().collect(),
        root: suzatary_core::cast::keys::root(),
    };
    let read =
        suzatary_core::chain_set::Read::of_stores([(&policy, &store as &dyn Prodrome<TodoEvent>)]);
    let payload = suzatary_core::routes::releases(&plan(), &read.set(day(5))).expect("answers");
    assert_eq!(payload.reads, "1.1");
    let two = &payload.releases[0];
    let held = two.held.as_ref().expect("the release is on the chain");
    assert_eq!(
        (two.todo.as_str(), held.title.as_str()),
        ("two", "Suzatary 2.0")
    );
    assert_eq!((two.lot, two.place), (Lot::Open, Place::Next));
    assert!(held.row.value.is_some());
    let items: Vec<(&str, Option<&str>, Standing, Named)> = two
        .items
        .iter()
        .map(|i| {
            (
                i.todo.as_str(),
                i.held.as_ref().map(|h| h.title.as_str()),
                i.standing,
                i.named,
            )
        })
        .collect();
    assert_eq!(
        items,
        vec![
            (
                "alpha",
                Some("the alpha item"),
                Standing::Counted,
                Named::Current
            ),
            (
                "bravo",
                Some("the bravo item"),
                Standing::Open,
                Named::Current
            ),
        ]
    );
    // 2.1 is declared and not on the chain yet: no row is made up for it.
    assert!(payload.releases[1].held.is_none());

    let elsewhere =
        suzatary_core::routes::releases(&plan(), &suzatary_core::chain_set::ChainSet::new(day(5)))
            .expect("answers");
    assert!(elsewhere.releases.is_empty());
}

/// An entry a PR branched before the last release wrote under Unreleased is merged into the
/// section that release closed; the cut moves it back, under its heading, and leaves what the
/// release itself held.
#[test]
fn entries_merged_into_a_released_section_are_unreleased_again() {
    let base = "# Changelog\n\n## [Unreleased]\n\n## [1.11.0] - 2026-10-12\n\n- Completes `todo:suzatary/x`: x\n\n### Added\n- a\n\n## [1.10.0] - 2026-10-01\n\n- old\n";
    let merged = "# Changelog\n\n## [Unreleased]\n\n## [1.11.0] - 2026-10-12\n\n- Completes `todo:suzatary/x`: x\n\n### Added\n- a\n- b\n  more of b\n\n### Fixed\n- c\n\n## [1.10.0] - 2026-10-01\n\n- old\n";
    assert_eq!(
        suzatary_core::release::reopened(base, merged),
        "# Changelog\n\n## [Unreleased]\n\n### Added\n- b\n  more of b\n\n### Fixed\n- c\n\n## [1.11.0] - 2026-10-12\n\n- Completes `todo:suzatary/x`: x\n\n### Added\n- a\n\n## [1.10.0] - 2026-10-01\n\n- old\n"
    );
    // A tree whose merges added nothing below Unreleased is left as it is.
    assert_eq!(suzatary_core::release::reopened(base, base), base);
    // An entry already under Unreleased joins the one moved there, under one heading.
    let fresh = merged.replace(
        "## [Unreleased]\n\n",
        "## [Unreleased]\n\n### Added\n- d\n\n",
    );
    let moved = suzatary_core::release::reopened(base, &fresh);
    assert!(
        moved.starts_with("# Changelog\n\n## [Unreleased]\n\n### Added\n- d\n- b\n  more of b\n\n### Fixed\n- c\n\n## [1.11.0]"),
        "{moved}"
    );
}

/// A PR branched two releases back lands its entry in the older released section; the cut moves
/// it back too, and leaves both sections as `base` held them.
#[test]
fn entries_merged_into_any_released_section_are_unreleased_again() {
    let base = "## [Unreleased]\n\n## [1.12.0] - 2026-10-13\n\n### Added\n- twelve\n\n## [1.11.0] - 2026-10-12\n\n### Fixed\n- eleven\n";
    let merged = "## [Unreleased]\n\n## [1.12.0] - 2026-10-13\n\n### Added\n- twelve\n\n## [1.11.0] - 2026-10-12\n\n### Fixed\n- eleven\n- late\n";
    assert_eq!(
        suzatary_core::release::reopened(base, merged),
        "## [Unreleased]\n\n### Fixed\n- late\n\n## [1.12.0] - 2026-10-13\n\n### Added\n- twelve\n\n## [1.11.0] - 2026-10-12\n\n### Fixed\n- eleven\n"
    );
}

/// A version set by hand and never released folds into the next release: the first release is
/// what the chain reads, `major.minor.0`, whatever the hand-set number said, and its section is
/// the new release's. After it, each release is the last one's next.
#[test]
fn an_unreleased_version_folds_into_the_first_release() {
    let placeholder = Version {
        major: 1,
        minor: 10,
        patch: 0,
    };
    let read = Counted { major: 1, minor: 9 };
    let version = Version::after(None, read).expect("nothing was released before");
    assert_eq!(
        version,
        Version {
            major: 1,
            minor: 9,
            patch: 0
        }
    );
    assert_eq!(
        Version::after(Some(version), read),
        Ok(Version {
            major: 1,
            minor: 9,
            patch: 1
        })
    );
    assert!(Version::after(Some(version), Counted { major: 1, minor: 8 }).is_err());
    let cut = suzatary_core::release::Cut {
        version,
        completes: Completes::default(),
        folds: Some(placeholder),
    };
    let changelog = "## [Unreleased]\n\n## [1.10.0] - 2026-10-12\n\nWhy.\n\n### Added\n- a\n\n## [0.17.0] - 2026-09-29\n";
    let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 14).expect("a day");
    assert_eq!(
        cut.changelog(changelog, day, "suzatary").expect("closes"),
        "## [Unreleased]\n\n## [1.9.0] - 2026-10-14\n\nWhy.\n\n### Added\n- a\n\n## [0.17.0] - 2026-09-29\n"
    );
    let entries = "## [Unreleased]\n\n### Fixed\n- b\n\n## [1.10.0] - 2026-10-12\n\nWhy.\n";
    assert_eq!(
        cut.changelog(entries, day, "suzatary").expect("closes"),
        "## [Unreleased]\n\n## [1.9.0] - 2026-10-14\n\n### Fixed\n- b\n\nWhy.\n",
        "one blank line where the folded heading stood"
    );
}

/// A cut rewrites the release file's data lines and keeps its leading comment as it is, so a
/// binary whose own words for that comment differ (an older revision cutting a newer tree)
/// leaves the header the tree holds; a file with no comment gets none.
#[test]
fn a_cut_keeps_the_header_and_rewrites_the_data() {
    let old = "# An example header.\n# Its second line.\n\nversion = \"1.2.0\"\ncompletes = [\"oldtodo\"]\n";
    let released = Released::new(
        Version {
            major: 1,
            minor: 3,
            patch: 0,
        },
        [TodoId::new("newtodo").expect("an id")],
    );
    let printed = released.print_over(old);
    assert_eq!(
        printed,
        "# An example header.\n# Its second line.\n\nversion = \"1.3.0\"\ncompletes = [\"newtodo\"]\n"
    );
    assert_eq!(
        released.print_over(&printed),
        printed,
        "a second cut moves nothing"
    );
    assert_eq!(
        released.print_over("version = \"1.2.0\"\n"),
        "version = \"1.3.0\"\ncompletes = [\"newtodo\"]\n"
    );
    assert_eq!(Released::parse(&printed), Ok(released));
}

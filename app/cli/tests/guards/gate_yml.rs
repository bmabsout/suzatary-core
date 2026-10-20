//! The tree half on GitHub's runners (`scripts/train.sh gate --remote github`): the steps the box
//! runs (`box_steps` in `scripts/train.sh`) and the jobs of `.github/workflows/gate.yml` hold every
//! step of `scripts/merge-gate.sh`'s tree half between them; the workflow runs on a push to a gate
//! branch alone, with a read-only token and every action pinned; and `scripts/e2e-part.sh` deals
//! the browser suite's files to parts that hold each once.

use super::merge_gate::{MADE_AND_READ, steps};
use super::{Scratch, repo};
use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The value of `<key>: '<value>'` (or unquoted, up to a comma or brace) in a flow mapping line.
fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.split_once(&format!("{key}: "))?.1;
    Some(match rest.strip_prefix('\'') {
        Some(quoted) => quoted.split_once('\'')?.0,
        None => rest.split([',', '}']).next()?.trim(),
    })
}

/// gate.yml's jobs, by lane: each one's steps and its e2e part.
fn jobs() -> BTreeMap<String, (Vec<String>, String)> {
    read(".github/workflows/gate.yml")
        .lines()
        .filter(|line| line.trim_start().starts_with("- { lane: "))
        .map(|line| {
            let lane = field(line, "lane").expect("a lane").to_owned();
            let steps = field(line, "steps")
                .expect("its steps")
                .split(',')
                .map(str::to_owned)
                .collect();
            let part = field(line, "part").expect("its part").to_owned();
            (lane, (steps, part))
        })
        .collect()
}

/// The steps `scripts/train.sh` runs on the box before the push.
fn box_steps() -> Vec<String> {
    read("scripts/train.sh")
        .lines()
        .find_map(|line| line.strip_prefix("box_steps=\""))
        .and_then(|list| list.strip_suffix('"'))
        .expect("train.sh names the box's steps")
        .split(',')
        .map(str::to_owned)
        .collect()
}

/// Every step of the tree half runs on the box or in a job, never both; each job and the box hold
/// what each step they run reads from another; and a step runs in more than one job only as what
/// a job's own reader reads, or as the e2e parts, each a part of its scenes.
#[test]
fn gate_yml_and_the_box_run_every_tree_step() {
    let tree: BTreeSet<String> = steps()
        .into_iter()
        .filter(|s| s.half == "tree")
        .map(|s| s.name)
        .collect();
    let jobs = jobs();
    assert!(!jobs.is_empty(), "gate.yml names its jobs");
    let on_box: BTreeSet<String> = box_steps().into_iter().collect();
    let in_jobs: BTreeSet<String> = jobs.values().flat_map(|(s, _)| s.clone()).collect();
    for name in on_box.iter().chain(&in_jobs) {
        assert!(tree.contains(name), "{name} is no step of the tree half");
    }
    assert_eq!(
        on_box.union(&in_jobs).cloned().collect::<BTreeSet<_>>(),
        tree,
        "the box and the jobs run every step of the tree half"
    );
    assert_eq!(
        on_box.intersection(&in_jobs).count(),
        0,
        "no step runs both on the box and in a job"
    );
    let places = jobs
        .iter()
        .map(|(lane, (s, _))| (lane.as_str(), s.clone()))
        .chain([("the box", on_box.iter().cloned().collect())]);
    for (place, held) in places {
        for &(made, reader) in MADE_AND_READ {
            assert!(
                !held.iter().any(|s| s == reader) || held.iter().any(|s| s == made),
                "{place} runs {reader}, which reads what {made} makes, so it runs {made} too"
            );
        }
    }
    for name in &in_jobs {
        let holding: Vec<&str> = jobs
            .iter()
            .filter(|(_, (s, _))| s.contains(name))
            .map(|(lane, _)| lane.as_str())
            .collect();
        let read_there = |lane: &str| {
            MADE_AND_READ
                .iter()
                .any(|&(made, reader)| made == name && jobs[lane].0.iter().any(|s| s == reader))
        };
        let unread: Vec<&&str> = holding.iter().filter(|lane| !read_there(lane)).collect();
        assert!(
            unread.len() <= 1 || unread.iter().all(|lane| !jobs[**lane].1.is_empty()),
            "{name} runs in {holding:?}, more than once for itself and not in parts"
        );
    }
}

/// The e2e jobs are the parts 1/n to n/n of one n, each once, and only they run e2e.
#[test]
fn gate_yml_e2e_jobs_are_every_part_once() {
    let jobs = jobs();
    let parts: Vec<&str> = jobs
        .values()
        .filter(|(s, _)| s.iter().any(|s| s == "e2e"))
        .map(|(_, part)| part.as_str())
        .collect();
    let n = parts.len();
    assert!(n > 1, "the scenes are split: {parts:?}");
    let want: BTreeSet<String> = (1..=n).map(|k| format!("{k}/{n}")).collect();
    assert_eq!(
        parts
            .iter()
            .map(|p| (*p).to_owned())
            .collect::<BTreeSet<_>>(),
        want
    );
    assert!(
        jobs.values()
            .all(|(s, part)| s.iter().any(|s| s == "e2e") || part.is_empty()),
        "a part only on an e2e job"
    );
}

/// The workflow runs on a push to `gate/**` and on nothing else (no pull request event, from a
/// fork or not), its token reads only, its checkout keeps no credential, every action is pinned
/// by its commit, and the one secret it names is handed to the Cachix action alone.
#[test]
fn gate_yml_runs_on_a_gate_branch_alone_with_a_read_only_token() {
    let text = read(".github/workflows/gate.yml");
    let body: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    let on = body
        .iter()
        .skip_while(|line| **line != "on:")
        .skip(1)
        .take_while(|line| line.starts_with(' ') || line.is_empty())
        .map(|line| line.trim())
        .collect::<Vec<_>>();
    assert_eq!(on, ["push:", "branches: [ 'gate/**' ]", ""], "{on:?}");
    let body = body.join("\n");
    assert!(!body.contains("pull_request"), "no pull request event");
    assert!(
        body.contains("permissions:\n  contents: read\n"),
        "the token reads only"
    );
    assert!(!body.contains(": write"), "and no job asks to write");
    assert!(body.contains("persist-credentials: false"));
    for line in body.lines().filter(|l| l.contains("uses: ")) {
        let pin = line
            .rsplit_once('@')
            .map(|(_, at)| at.split_whitespace().next());
        assert!(
            pin.flatten()
                .is_some_and(|sha| sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())),
            "pinned by commit: {line}"
        );
    }
    assert_eq!(body.matches("secrets.").count(), 1, "one secret");
    let cachix = body
        .split("- uses: ")
        .find(|part| part.starts_with("cachix/cachix-action@"))
        .expect("the Cachix action");
    assert!(cachix.contains("authToken: ${{ secrets.CACHIX_AUTH_TOKEN }}"));
}

/// actionlint passes the workflow: its syntax, its expressions and its runner labels.
#[test]
fn gate_yml_passes_actionlint() {
    let out = Command::new("actionlint")
        .arg(".github/workflows/gate.yml")
        .current_dir(repo())
        .output()
        .expect("actionlint runs (the .#triage shell carries it)");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `scripts/e2e-part.sh <k>/<n>`: the n parts hold every file once, the same files on every run,
/// each part some file while there are files enough, and a part that is no part is refused.
#[test]
fn e2e_parts_hold_every_file_once() {
    let scratch = Scratch::new("e2e-part");
    let e2e = scratch.join("web/e2e");
    std::fs::create_dir_all(&e2e).expect("web/e2e");
    let files: BTreeSet<String> = (0..11).map(|i| format!("scene{i}")).collect();
    for (i, name) in files.iter().enumerate() {
        std::fs::write(
            e2e.join(format!("{name}.test.ts")),
            "x".repeat(100 + 37 * (i % 5)),
        )
        .expect("a scene");
    }
    std::fs::write(e2e.join("bench.bench.ts"), "not a scene").expect("a bench");
    let part = |arg: &str| {
        let out = Command::new(repo().join("scripts/e2e-part.sh"))
            .arg(arg)
            .current_dir(&*scratch)
            .output()
            .expect("e2e-part.sh runs");
        (
            out.status.code(),
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        )
    };
    let mut seen = Vec::new();
    for k in 1..=4 {
        let (code, names) = part(&format!("{k}/4"));
        assert_eq!(code, Some(0));
        assert!(!names.is_empty(), "part {k} holds a file");
        assert_eq!(part(&format!("{k}/4")).1, names, "the same part again");
        seen.extend(names);
    }
    seen.sort();
    assert_eq!(
        seen,
        files.into_iter().collect::<Vec<_>>(),
        "each file once"
    );
    for bad in ["5/4", "0/4", "1", "a/b", ""] {
        assert_eq!(part(bad).0, Some(64), "{bad:?} is refused");
    }
}

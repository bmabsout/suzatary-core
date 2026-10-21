//! What suzatary-core's runners build from a publication, pinned from the workflow's source:
//! the deploy's builds pushed to the binary cache (`publication.yml`), reached only by the
//! owner's own push to master, measured against the cache's budget before they are pushed, and
//! nothing built from the deployment's own names. It is the one workflow that writes to the
//! cache.

use super::repo;

fn workflow(name: &str) -> String {
    std::fs::read_to_string(repo().join(".github/workflows").join(name))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The text from `start` up to the next non-blank line indented no deeper than it: one YAML
/// key's block, comments before the next key excluded.
fn section<'a>(text: &'a str, start: &str) -> &'a str {
    let from = text.find(start).unwrap_or_else(|| panic!("no {start:?}"));
    let indent = start.len() - start.trim_start().len();
    let rest = &text[from + start.len()..];
    let to = rest
        .lines()
        .scan(0, |at, line| {
            let here = *at;
            *at += line.len() + 1;
            Some((here, line))
        })
        .skip(1)
        .find(|(_, line)| {
            let trimmed = line.trim_start();
            !trimmed.is_empty() && line.len() - trimmed.len() <= indent
        })
        .map_or(rest.len(), |(at, _)| at);
    &text[from..from + start.len() + to]
}

/// The publication cache runs on a push to master in the public repository alone, never on a
/// pull request, and its token is handed only to the last step, after every build.
#[test]
fn publication_cache_runs_only_on_the_owners_push_to_master() {
    let text = workflow("publication.yml");
    assert_eq!(
        section(&text, "\non:").trim(),
        "on:\n  push:\n    branches: [master]",
        "a push to master and nothing else"
    );
    assert!(text.contains("if: ${{ !github.event.repository.private }}"));
    assert!(
        text.contains("    environment: cachix\n"),
        "the token is the master-only environment's"
    );
    assert_eq!(text.matches("secrets.").count(), 1, "one secret, once");
    let last = text.rfind("      - name:").expect("steps");
    assert!(
        text[last..].contains("CACHIX_AUTH_TOKEN: ${{ secrets.CACHIX_AUTH_TOKEN }}"),
        "the token is the last step's alone"
    );
    for attr in [".#suzatary", ".#suzatary-web", ".#suzatary-plugins"] {
        assert!(text.contains(&format!("{attr}\n")), "builds {attr}");
    }
    // The APK bakes in the deployment's box name and Firebase project, read from the
    // environment under --impure alone: a public cache holds nothing built that way.
    let code: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    for never in ["--impure", "PORTAL", "apk", "vars."] {
        assert!(!code.contains(never), "builds nothing with {never}");
    }
}

/// What the push adds is measured before it, and over 1 GB fails the step before the push, so
/// the push never runs; the push sends the measured paths.
#[test]
fn publication_cache_measures_its_push_and_refuses_over_a_gigabyte() {
    let text = workflow("publication.yml");
    let measure = section(&text, "      - name: Measure what the push adds");
    let push = section(&text, "      - name: Push the builds to the cache");
    assert!(
        text.find(measure) < text.find(push),
        "measured before it is pushed"
    );
    assert!(measure.contains("LIMIT: 1000000000\n"), "a limit of 1 GB");
    assert!(
        measure.contains("if ((bytes > LIMIT)); then") && measure.contains("exit 1\n"),
        "fails over it"
    );
    assert!(measure.contains("echo \"the push adds "), "prints the size");
    assert!(!measure.contains("continue-on-error") && !push.contains("if:"));
    assert!(
        push.contains("cachix -- push bmabsout <unserved.txt"),
        "pushes what was measured"
    );
    assert!(measure.contains(">unserved.txt"));
}

/// No workflow but publication.yml names the Cachix's token or pushes to a binary cache.
#[test]
fn publication_yml_alone_writes_to_the_cachix() {
    let dir = repo().join(".github/workflows");
    let mut writers = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the workflows") {
        let name = entry
            .expect("a workflow")
            .file_name()
            .to_string_lossy()
            .into_owned();
        let text = workflow(&name);
        if [
            "CACHIX_AUTH_TOKEN",
            "cachix push",
            "cachix -- push",
            "cachix-action",
            "authToken",
        ]
        .iter()
        .any(|word| text.contains(word))
        {
            writers.push(name);
        }
    }
    assert_eq!(writers, ["publication.yml"]);
}

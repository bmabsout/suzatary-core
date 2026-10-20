use std::fmt::Write as _;

use prodrome_core::literal::Datetime;

use super::{Decision, DecisionId, Ledger};
use crate::escape::escape_markup;

/// The decisions as one Typst document: a section per lineage, newest first, each decision
/// above what it supersedes, with where it stands and what it is the newer word on.
///
/// Every title, rationale and reason is DATA and enters escaped; a body is markup the door
/// admitted, placed inside a block as it stands. So the document compiles in the closed world
/// as its parts did, and nothing a writer typed becomes structure here.
#[must_use]
pub fn document(ledger: &Ledger) -> String {
    let standing = ledger.iter().filter(|(id, _)| ledger.in_force(id)).count();
    let mut out = format!(
        "= Decisions\n\n{} recorded, {standing} in force. A decision stands above what it \
         supersedes.\n",
        ledger.len()
    );
    let cycles = ledger.cycles();
    for lineage in ledger.lineages() {
        out.push_str("\n#line(length: 100%)\n");
        for id in lineage {
            let Some(decision) = ledger.get(id) else {
                continue;
            };
            section(&mut out, ledger, id, decision, cycles.contains(id));
        }
    }
    out
}

fn section(out: &mut String, ledger: &Ledger, id: &DecisionId, decision: &Decision, cycle: bool) {
    let statement = decision.statement();
    let _ = write!(
        out,
        "\n== {}\n\n`{id}`, decided {}. {}\n",
        escape_markup(statement.title()),
        day(decision.decided),
        standing(ledger, id, decision)
    );
    if cycle {
        out.push_str("\n*On a cycle of links:* two writers each superseded the other; the next write settles it.\n");
    }
    if decision.is_conflict() {
        let _ = write!(
            out,
            "\n*In conflict:* {} statements stand; the first is shown.\n",
            decision.statements().len()
        );
    }
    let _ = write!(
        out,
        "\n#block(inset: (left: 1em))[{}]\n\n_Why:_ {}\n",
        statement.body().as_str(),
        escape_markup(statement.rationale())
    );
    if !decision.supersedes.is_empty() {
        let _ = write!(out, "\nSupersedes {}.\n", ids(decision.supersedes.iter()));
    }
}

fn standing(ledger: &Ledger, id: &DecisionId, decision: &Decision) -> String {
    let by = ledger.superseded_by(id);
    let mut said = Vec::new();
    if let Some(withdrawn) = &decision.withdrawn {
        said.push(format!(
            "*Withdrawn* {}: {}",
            day(withdrawn.at),
            escape_markup(&withdrawn.reason)
        ));
    }
    if !by.is_empty() {
        said.push(format!("*Superseded* by {}", ids(by.into_iter())));
    }
    if said.is_empty() {
        "*In force.*".to_owned()
    } else {
        format!("{}.", said.join("; "))
    }
}

fn ids<'a>(ids: impl Iterator<Item = &'a DecisionId>) -> String {
    ids.map(|id| format!("`{id}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn day(at: Datetime) -> String {
    format!("{:04}-{:02}-{:02}", at.year(), at.month(), at.day())
}

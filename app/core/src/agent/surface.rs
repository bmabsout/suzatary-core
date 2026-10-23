//! The triage agent's verbs, as one closed sum.
//!
//! The agent re-binds these names every run with no negotiation, so a name leaving [`TriageVerb`] is a
//! major version and one joining it a minor; `app/core/tests/mcp-surface.json` pins [`tools`]
//! byte for byte, and `confirm` and `send` are pinned never to appear.

use serde_json::Value;

use super::verbs::{
    Context, Evaluate, HistoryArgs, Mark, New, NoiseAdd, RecordArgs, ShowArgs, Sync, TodoAdd,
    TodoUpdate, VerifyChain,
};
use super::{Call, Holds, Says};
use crate::authoring::Writer;
use crate::capability::Agent;
use crate::chains::ChainPolicy;
use crate::verb::{Chained, Surface, verbs};

/// The triage agent's verbs: each runs against a [`Call`] and answers JSON.
pub enum TriageSurface {}

impl Surface for TriageSurface {
    type Call<'a> = Call<'a>;
    type Answer = Value;
    type Class = ();
}

impl Holds for TriageSurface {
    const WRITES_AS: Writer = super::WRITES_AS;
    const SAYS: Says = Says {
        list_open_todos: "Every currently-open todo, as {id, kind, category, priority, body, \
        waiting_on, authored_by}. Use this to avoid creating a duplicate — if a \
        message belongs to a topic already tracked here, link it to that todo's \
        id via `mark ... --todo-id <id>` instead of authoring a new one. \
        Complete view: it is the chain's own fold, not a scan of recent files, \
        and the priority is evaluated as of right now rather than whenever a \
        document was last compiled.\n\n\
        ⚠️ IT IS ONE CHAIN'S LIST. Dedup against the chain you are about to \
        write to: leaving `chain` out reads the owner's own todos, and \
        `chain=\"suzatary\"` reads this system's own work. Checking one and \
        writing to the other is how a duplicate gets made.",
        git_diff_summary: "A short --stat summary of uncommitted changes, for a final sanity check \
        before ending your session. You do not commit or push yourself — that \
        happens afterward, deterministically, only if every store verifies.",
        history: "The full lifecycle event stream for one todo from the chain, oldest \
        first, as readable text (one line per event: kind, at, actor, note). An \
        unknown id is not an error — it just has no history yet. `chain` picks \
        the store the todo lives in.",
        todo_show: "One todo as the chain holds it: its rendered block, its fields, and who \
        last authored it when. Use it to check what you just wrote, or to see a \
        todo's detail and source before deciding whether a new message belongs to \
        it. `chain` picks the store it lives in.",
        show: "Full message headers + plain-text body. Takes either the `id` field from \
        `new`/`search`, or a canonical `hash` — full, or a unique prefix such as \
        the one stored in a todo's `source.hash`. A body longer than one answer \
        comes back as its first part, with a line naming how many parts there are \
        — pass `part` (2, 3, …) for the rest. Never guess at what a cut part left \
        out; ask for the next one. `links` and every header travel whole on every \
        part.",
        record: "Record a lifecycle event for a todo onto the append-only chain. This is \
        your ONLY write to history. kind is 'completed', 'cancelled', \
        'reopened', or 'tended' (no 'created' — todo authoring lives in the \
        content verbs; no spec authoring). 'tended' is a pass at a RECURRING \
        todo (its price holds a Recur): it was done again and stays open, so \
        its price restarts from now. Never 'completed' for one of those.\n\nThe actor is always 'triage' and the timestamp is \
        always the server's clock — you cannot set either, and you cannot \
        backdate. Your events are PROVISIONAL by construction: they do not bind \
        evaluation until the owner confirms and a trusted actor re-appends them. \
        `todo` must be an id the chain already knows (some prior event mentions \
        it); an unknown id is rejected as a likely typo. A todo you authored this \
        run with `todo_add` is already known.\n\n\
        `chain` is WHICH STORE the todo lives in and must match the one it was \
        authored in — leave it out for the owner's own list, pass \
        `chain=\"suzatary\"` for this system's work.",
    };

    fn agent<'a>(call: &Self::Call<'a>) -> Agent<'a> {
        *call.caps
    }

    fn chain<'a>(call: &Self::Call<'a>) -> &'a ChainPolicy {
        call.chain
    }
}

type GitDiffSummary = super::verbs::GitDiffSummary<TriageSurface>;
type History = super::verbs::History<TriageSurface, Chained<HistoryArgs>>;
type ListOpenTodos = super::verbs::ListOpenTodos<TriageSurface>;
type Record = super::verbs::Record<TriageSurface, Chained<RecordArgs>>;
type Search = super::verbs::Search<TriageSurface>;
type Show = super::verbs::Show<TriageSurface, ShowArgs>;
type TodoShow = super::verbs::TodoShow<TriageSurface>;

verbs! {
    /// Every verb the triage agent may call, and nothing else.
    pub enum TriageVerb;
    /// One call of a [`TriageVerb`], its arguments parsed.
    pub enum TriageCall;
    on TriageSurface;
    Sync: (),
    New: (),
    Search: (),
    Show: (),
    Mark: (),
    NoiseAdd: (),
    ListOpenTodos: (),
    GitDiffSummary: (),
    Record: (),
    History: (),
    Evaluate: (),
    VerifyChain: (),
    Context: (),
    TodoAdd: (),
    TodoUpdate: (),
    TodoShow: (),
}

/// The whole surface as JSON in the MCP `tools/list` shape, which both the server and the
/// snapshot read.
#[must_use]
pub fn tools() -> Value {
    Value::Array(TriageVerb::ALL.iter().map(|verb| verb.tool()).collect())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The surface is a closed list, and `confirm` and `send` may never join it.
    #[test]
    fn there_is_no_confirm_verb_and_no_send_verb() {
        for verb in TriageVerb::ALL.iter().map(|verb| verb.name()) {
            assert!(
                !verb.contains("confirm"),
                "an agent that can confirm its own writes has confirmed nothing: {verb}",
            );
            assert!(
                !verb.contains("send") && !verb.contains("reply"),
                "sending is structurally human-only: {verb}",
            );
        }
    }

    /// `record` is the only mutator over the chain's lifecycle.
    #[test]
    fn the_chain_has_exactly_one_lifecycle_mutator() {
        let names: BTreeSet<&str> = TriageVerb::ALL.iter().map(|verb| verb.name()).collect();
        assert!(names.contains("record"));
        for forbidden in ["append", "seal", "delete", "amend"] {
            assert!(!names.contains(forbidden), "{forbidden}");
        }
    }

    #[test]
    fn every_verb_is_named_once() {
        let names: BTreeSet<&str> = TriageVerb::ALL.iter().map(|verb| verb.name()).collect();
        assert_eq!(names.len(), TriageVerb::ALL.len());
    }

    /// `named` inverts `name`, and the tool a model is shown carries the same name.
    #[test]
    fn a_verb_is_named_by_its_name_and_its_tool() {
        for &verb in TriageVerb::ALL {
            assert_eq!(TriageVerb::named(verb.name()), Some(verb));
            assert_eq!(verb.tool()["name"], verb.name());
        }
    }
}

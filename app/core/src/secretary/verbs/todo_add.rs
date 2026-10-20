use schemars::JsonSchema;
use serde::Deserialize;

use serde_json::Value;

use crate::agent::priced::Priced;
use crate::agent::sourced::Sourced;
use crate::agent::writes::{self, Adding};
use crate::refusal::Refusal;
use crate::secretary::{Call, SecretarySurface, WRITES_AS};
use crate::verb::{Caught, Chained, Verb};

/// Author a todo, as the trusted assistant.
pub struct TodoAdd;

#[derive(Deserialize, JsonSchema)]
pub struct TodoAddArgs {
    todo_id: String,
    body: String,
    rationale: String,
    #[serde(default = "crate::todo::fields::plain_kind")]
    kind: String,
    category: Option<String>,
    #[serde(default)]
    priority: Caught<Option<String>>,
    #[serde(default)]
    deadline_end_date: Caught<Option<String>>,
    #[serde(default)]
    deadline_start_date: Caught<Option<String>>,
    #[serde(default)]
    deadline_start_priority: Caught<Option<String>>,
    #[serde(default)]
    deadline_end_priority: Caught<Option<String>>,
    #[serde(default)]
    deadline_lead_up_weeks: Caught<Option<usize>>,
    #[serde(default)]
    recur_every_days: Caught<Option<usize>>,
    #[serde(default)]
    periodic_every_days: Caught<Option<usize>>,
    #[serde(default)]
    cycle_from_priority: Caught<Option<String>>,
    #[serde(default)]
    cycle_to_priority: Caught<Option<String>>,
    detail: Option<String>,
    waiting_on: Option<String>,
    date: Option<String>,
    #[serde(default)]
    source_sender: Caught<Option<String>>,
    #[serde(default)]
    source_subject: Caught<Option<String>>,
    #[serde(default)]
    source_date: Caught<Option<String>>,
    #[serde(default)]
    source_hash: Caught<Option<String>>,
    parent: Option<String>,
}

impl TodoAddArgs {
    fn adding(self) -> Adding {
        Adding {
            todo_id: self.todo_id,
            body: self.body,
            rationale: self.rationale,
            kind: self.kind,
            category: self.category,
            priced: Priced {
                priority: self.priority,
                deadline_end_date: self.deadline_end_date,
                deadline_start_priority: self.deadline_start_priority,
                deadline_end_priority: self.deadline_end_priority,
                deadline_start_date: self.deadline_start_date,
                deadline_lead_up_weeks: self.deadline_lead_up_weeks,
                recur_every_days: self.recur_every_days,
                periodic_every_days: self.periodic_every_days,
                cycle_from_priority: self.cycle_from_priority,
                cycle_to_priority: self.cycle_to_priority,
            },
            detail: self.detail.unwrap_or_default(),
            waiting_on: self.waiting_on,
            sourced: Sourced {
                sender: self.source_sender,
                subject: self.source_subject,
                date: self.source_date,
                hash: self.source_hash,
            },
            date: self.date,
            parent: self.parent,
        }
    }
}

impl Verb for TodoAdd {
    type Surface = SecretarySurface;
    type Args = Chained<TodoAddArgs>;
    type Answer = Value;
    const NAME: &'static str = "todo_add";
    const DESCRIPTION: &'static str = "AUTHOR A TODO, as the trusted actor `assistant` — so it binds the fold \
        and renders with no `(unconfirmed)` marker, unlike the triage agent's. \
        That is affordable because the owner is reading this conversation, so \
        author what they asked for and propose the rest.\n\n\
        Pass PLAIN PROSE and escape nothing. `priority` is a FULFILLMENT, \
        0-100%: LOW is urgent, HIGH is satisfied or someday. A deadline bounds \
        WHEN, not the stakes — a trivial dated item stays at 60%+ on its last \
        day. `rationale` is required: one sentence saying why THAT number.\n\n\
        A chore done again and again takes `recur_every_days` with \
        `cycle_from_priority` and `cycle_to_priority`: after every pass (`record` \
        kind 'tended') it runs from the first to the second over that many days, \
        then holds, and before the first pass it keeps the price it had. \
        `periodic_every_days` is the same cycle on the calendar, whatever is \
        done. Give one price: `priority`, `deadline_*`, or a cycle.\n\n\
        `chain` picks the store: leave it out for the owner's life, \
        `chain=\"suzatary\"` for this system's own work.\n\n\
        `parent` makes it a SUBTODO: the parent's price gains a reference to \
        it in the same write, so the parent's value composes this one's; \
        without `category` it takes the parent's.";

    fn run(call: &Call<'_>, Chained(args): Chained<TodoAddArgs>) -> Result<Value, Refusal> {
        writes::todo_add(
            &call.caps.agent(),
            call.deployment,
            call.chain,
            WRITES_AS,
            args.adding(),
        )
    }
}

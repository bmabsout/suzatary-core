use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::agent::priced::Priced;
use crate::agent::writes::Updating;
use crate::agent::{Call, TriageSurface, WRITES_AS, writes};
use crate::refusal::Refusal;
use crate::verb::{Caught, Chained, Verb};

/// Change fields on a todo.
pub struct TodoUpdate;

#[derive(Deserialize, JsonSchema)]
pub struct TodoUpdateArgs {
    todo_id: String,
    rationale: Option<String>,
    body: Option<String>,
    detail: Option<String>,
    detail_append: Option<String>,
    waiting_on: Option<String>,
    #[serde(default)]
    clear_waiting_on: bool,
    #[serde(default)]
    priority: Caught<Option<String>>,
    #[serde(default)]
    deadline_end_date: Caught<Option<String>>,
    #[serde(default = "crate::agent::priced::start_priority")]
    deadline_start_priority: Caught<String>,
    #[serde(default = "crate::agent::priced::end_priority")]
    deadline_end_priority: Caught<String>,
    #[serde(default)]
    deadline_start_date: Caught<Option<String>>,
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
}

impl TodoUpdateArgs {
    fn updating(self) -> Updating {
        Updating {
            todo_id: self.todo_id,
            rationale: self.rationale,
            priced: Priced {
                priority: self.priority,
                deadline_end_date: self.deadline_end_date,
                deadline_start_priority: self.deadline_start_priority.map(Some),
                deadline_end_priority: self.deadline_end_priority.map(Some),
                deadline_start_date: self.deadline_start_date,
                deadline_lead_up_weeks: self.deadline_lead_up_weeks,
                recur_every_days: self.recur_every_days,
                periodic_every_days: self.periodic_every_days,
                cycle_from_priority: self.cycle_from_priority,
                cycle_to_priority: self.cycle_to_priority,
            },
            body: self.body,
            detail: self.detail,
            detail_append: self.detail_append,
            waiting_on: self.waiting_on,
            clear_waiting_on: self.clear_waiting_on,
        }
    }
}

impl Verb for TodoUpdate {
    type Surface = TriageSurface;
    type Args = Chained<TodoUpdateArgs>;
    type Answer = Value;
    const NAME: &'static str = "todo_update";
    const DESCRIPTION: &'static str = "Change fields on a todo that already exists. Only the fields you pass are \
        touched; everything else keeps its current value.\n\nChanging `priority` \
        (or a deadline, or a cycle — the same price fields `todo_add` takes, \
        one of them) REQUIRES `rationale`. The comment above that number is \
        the only record of why it is what it is, and leaving the old sentence \
        beside a new number misrepresents it as reviewed reasoning. Passing \
        `rationale` alone rewrites just the reason.\n\n`clear_waiting_on` sets it \
        back to `none` — use it the moment there is an email we could send, since \
        `waitingOn` means nothing we can do.\n\n`body`, `detail` and \
        `detail_append` are Typst markup, checked as `todo_add` checks them.\n\n\
        `detail_append`: a sentence \
        to add under the existing detail, keeping its markup — use this, \
        never `detail`, when the detail already exists; give at most one of \
        the two.\n\n`chain` must be the store the todo was authored in; there is \
        no moving one between them.";

    fn run(call: &Call<'_>, Chained(args): Chained<TodoUpdateArgs>) -> Result<Value, Refusal> {
        writes::todo_update(
            call.caps,
            call.deployment.zone,
            call.chain,
            WRITES_AS,
            args.updating(),
        )
    }
}

use schemars::JsonSchema;
use serde::Deserialize;

use serde_json::Value;

use crate::agent::priced::Priced;
use crate::agent::writes::{self, Updating};
use crate::refusal::Refusal;
use crate::secretary::{Call, SecretarySurface, WRITES_AS};
use crate::verb::{Caught, Chained, Verb};

/// Change fields on a todo, as the trusted assistant.
pub struct TodoUpdate;

#[derive(Deserialize, JsonSchema)]
pub struct TodoUpdateArgs {
    todo_id: String,
    body: Option<String>,
    detail: Option<String>,
    detail_append: Option<String>,
    rationale: Option<String>,
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
    waiting_on: Option<String>,
    #[serde(default)]
    clear_waiting_on: bool,
}

impl TodoUpdateArgs {
    fn updating(self) -> Updating {
        Updating {
            todo_id: self.todo_id,
            rationale: self.rationale,
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
            body: self.body,
            detail: self.detail,
            detail_append: self.detail_append,
            waiting_on: self.waiting_on,
            clear_waiting_on: self.clear_waiting_on,
        }
    }
}

impl Verb for TodoUpdate {
    type Surface = SecretarySurface;
    type Args = Chained<TodoUpdateArgs>;
    type Answer = Value;
    const NAME: &'static str = "todo_update";
    const DESCRIPTION: &'static str = "Change fields on a todo that exists. Only what you name changes. \
        Changing the price (`priority`, the `deadline_*` fields, or a cycle, as \
        on todo_add) requires a fresh `rationale` — a reprice with no \
        reason loses the only thing no mechanism can recover. \
        `clear_waiting_on` unblocks, which is the usual update when a reply \
        resolves what a todo was waiting on. `detail_append`: a sentence to add \
        under the existing detail, keeping its markup — use this, never \
        `detail`, when the detail already exists; give at most one of the two.";

    fn run(call: &Call<'_>, Chained(args): Chained<TodoUpdateArgs>) -> Result<Value, Refusal> {
        writes::todo_update(
            &call.caps.agent(),
            call.deployment.zone,
            call.chain,
            WRITES_AS,
            args.updating(),
        )
    }
}

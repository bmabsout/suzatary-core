use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::agent::priced::Priced;
use crate::agent::sourced::Sourced;
use crate::agent::writes::Adding;
use crate::agent::{Call, TriageSurface, WRITES_AS, writes};
use crate::refusal::Refusal;
use crate::verb::{Caught, Chained, Verb};

/// Author a new todo.
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
    detail: String,
    waiting_on: Option<String>,
    #[serde(default)]
    source_sender: Caught<Option<String>>,
    #[serde(default)]
    source_subject: Caught<Option<String>>,
    #[serde(default)]
    source_date: Caught<Option<String>>,
    #[serde(default)]
    source_hash: Caught<Option<String>>,
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
    date: Option<String>,
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
                deadline_start_priority: self.deadline_start_priority.map(Some),
                deadline_end_priority: self.deadline_end_priority.map(Some),
                deadline_start_date: self.deadline_start_date,
                deadline_lead_up_weeks: self.deadline_lead_up_weeks,
                recur_every_days: self.recur_every_days,
                periodic_every_days: self.periodic_every_days,
                cycle_from_priority: self.cycle_from_priority,
                cycle_to_priority: self.cycle_to_priority,
            },
            detail: self.detail,
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
    type Surface = TriageSurface;
    type Args = Chained<TodoAddArgs>;
    type Answer = Value;
    const NAME: &'static str = "todo_add";
    const DESCRIPTION: &'static str = "Author a new todo. This is how you create todos — you do not edit \
        files.\n\n\
        `todo_id`: 3-64 lowercase letters/digits, starting with a letter. It is \
        the handle `mark --todo-id` and `record` use, so choose something you can \
        reproduce (e.g. `permitfiling`).\n\
        `body`: the action line, one sentence, as Typst MARKUP: `*bold*` is bold, and \
        a `$`, `@`, `#`, `*`, `_`, `<` or `[` you mean literally takes a backslash \
        (`Pay \\$40`). It is compiled before it is stored, without `html` or any \
        file, and refused with the line and column when it does not compile. \
        `detail` is the same. Every other field is plain text.\n\
        `rationale`: REQUIRED — WHY this priority, in one sentence. It is \
        recorded as the comment above the number and is the only place the \
        reasoning exists.\n\
        `kind`: 'todo' (needs `category`), 'ekaTodo' or 'suzateryTodo' (which set \
        their own category — do not pass one).\n\
        `priority`: a whole percentage like '65%'. It is a FULFILLMENT: LOW means \
        urgent, HIGH means satisfied or someday. Give exactly one of `priority`, \
        `deadline_end_date`, `recur_every_days` or `periodic_every_days`.\n\
        `deadline_*`: use when the todo decays toward a date rather than sitting \
        at a fixed number. A deadline bounds WHEN, not the stakes: something \
        merely moot afterwards (a form, a small recoverable cost) still floors \
        around 60% on its last day.\n\
        `recur_every_days`: a chore done again and again (vinegar the \
        toothbrush every two months). After every pass (`record` kind \
        'tended') the fulfillment runs from `cycle_from_priority` to \
        `cycle_to_priority` over that many days, then holds; before the first \
        pass it keeps the price it had. `periodic_every_days` is the same \
        cycle on the calendar, whatever is done (rent). Both need both ends.\n\
        `waiting_on`: only when there is genuinely nothing we can do. One \
        unblocking email we could send means leave it unset.\n\
        `source_*`: all four or none — the message this came from.\n\
        `date`: which day the todo is created on, YYYY-MM-DD, default today.\n\
        `parent`: the id of the todo this is a SUBTODO of. The parent's price \
        gains a reference to this one in the same write, so its value composes \
        this one's; without `category` this takes the parent's.\n\
        `chain`: WHICH LIST it goes in, and this is a decision, not a detail. \
        Leave it out for the owner's own todos — mail, deadlines, people, money, \
        travel. Pass `chain=\"suzatary\"` for this system's own work: a gate \
        that failed, a test that is flaky, a sync that broke, a verb that needs \
        fixing. The test is who the todo is FOR. A todo lives in ONE chain and \
        cannot be moved afterwards, because the Prodrome does not rewrite.";

    fn run(call: &Call<'_>, Chained(args): Chained<TodoAddArgs>) -> Result<Value, Refusal> {
        writes::todo_add(
            call.caps,
            call.deployment,
            call.chain,
            WRITES_AS,
            args.adding(),
        )
    }
}

use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::agent::{Holds, writes};
use crate::refusal::Refusal;
use crate::verb::{Own, Verb};

/// A todo's lifecycle event, written as the surface writes, its arguments spelled `A`.
pub struct Record<S, A>(PhantomData<(S, A)>);

/// The event, the todo and a note, empty when absent.
#[derive(Deserialize, JsonSchema)]
pub struct RecordArgs {
    kind: String,
    todo: String,
    #[serde(default)]
    note: String,
}

/// The same arguments with a note that may be null, absent meaning none.
#[derive(Deserialize, JsonSchema)]
pub struct RecordArgsNullNote {
    kind: String,
    todo: String,
    note: Option<String>,
}

impl Own<RecordArgs> for RecordArgsNullNote {
    fn own(self) -> RecordArgs {
        RecordArgs {
            kind: self.kind,
            todo: self.todo,
            note: self.note.unwrap_or_default(),
        }
    }
}

impl<S: Holds, A: Own<RecordArgs> + DeserializeOwned + JsonSchema> Verb for Record<S, A> {
    type Surface = S;
    type Args = A;
    type Answer = Value;
    const NAME: &'static str = "record";
    const DESCRIPTION: &'static str = S::SAYS.record;

    fn run(call: &S::Call<'_>, args: A) -> Result<Value, Refusal> {
        let args = args.own();
        writes::record(
            &S::agent(call),
            S::WRITES_AS,
            &args.kind,
            &args.todo,
            &args.note,
        )
    }
}

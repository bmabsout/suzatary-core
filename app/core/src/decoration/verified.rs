//! The containment check: a block is written only if its rendering reads back,
//! through typst's own parser, as the AST that went in. Fields can come from
//! attacker-controlled mail; escaping is a claim, this is a measurement, and a
//! field that altered the structure makes the two differ.
//!
//! It does not go through [`crate::capability::Render`]: `Render::html` takes an
//! [`crate::escape::Escaped`], which only confirmed content yields, and the
//! agent's writes are provisional. The leading `//` run is verified too, so an
//! update cannot silently drop a note about the whole block.

use super::value::{Call, render_block};
use super::{DecorationError, read};
use crate::refusal::Refusal;

/// Render a block and prove it reads back to itself, through
/// [`read::only_block`] so nothing after the closing paren is tolerated.
///
/// # Errors
///
/// A [`Refusal`] when the rendering does not read back as the block that was
/// built; the write does not happen.
pub fn render_verified(call: &Call, leading: &[String]) -> Result<String, Refusal> {
    let rendered = render_block(call, leading);
    let read = read::only_block(&rendered, &call.callee).map_err(|DecorationError(message)| {
        Refusal::bad_request(format!("refusing to write unreadable typst: {message}"))
    })?;
    if &read.call != call || read.leading != leading {
        return Err(Refusal::bad_request(
            "refusing to write: the rendered block does not read back as the \
             block that was built, which means a field altered the structure",
        ));
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::super::value::{Arg, Value};
    use super::*;

    fn a_call() -> Call {
        Call {
            callee: "todo".to_owned(),
            args: vec![
                Arg::named("id", Value::Str("filething".to_owned())),
                Arg::named("created", Value::Ident("note_day".to_owned())),
                Arg::explained(
                    "priority",
                    Value::Num("35.00%".to_owned()),
                    vec!["Missing it is not recoverable.".to_owned()],
                ),
                Arg::named("waitingOn", Value::Ident("none".to_owned())),
            ],
            trailing: vec![r"File the \*permit\* renewal".to_owned()],
            hashed: true,
            tail: Vec::new(),
        }
    }

    #[test]
    fn a_block_round_trips_through_its_own_rendering() {
        let call = a_call();
        let text = render_verified(&call, &[]).expect("verifies");
        let back = read::only_block(&text, "todo").expect("reads");
        assert_eq!(back.call, call);
        assert!(back.leading.is_empty());
    }

    /// Rendering is idempotent: layout is computed from the value alone.
    #[test]
    fn rendering_a_read_rendering_changes_nothing() {
        let once = render_block(&a_call(), &["a note".to_owned()]);
        let back = read::only_block(&once, "todo").expect("reads");
        assert_eq!(render_block(&back.call, &back.leading), once);
    }

    /// A bare `]` in a body would close the content block: escaped it round
    /// trips, raw it is refused.
    #[test]
    fn an_escaped_body_survives_and_a_raw_one_would_not() {
        let mut call = a_call();
        call.trailing = vec![
            crate::escape::escape_markup("close ] this #todo(x)[")
                .as_str()
                .to_owned(),
        ];
        let text = render_verified(&call, &[]).expect("verifies");
        assert_eq!(read::only_block(&text, "todo").expect("reads").call, call);

        let mut raw = a_call();
        raw.trailing = vec!["close ] this".to_owned()];
        assert!(render_verified(&raw, &[]).is_err());
    }

    /// A dropped leading run is a refusal, not a quiet deletion.
    #[test]
    fn the_leading_run_is_part_of_what_is_verified() {
        let text = render_verified(&a_call(), &["kept".to_owned()]).expect("verifies");
        assert!(text.starts_with("// kept\n"), "{text}");
        assert_eq!(
            read::only_block(&text, "todo").expect("reads").leading,
            vec!["kept".to_owned()]
        );
    }

    /// A multibyte body is still one field: the renderer counts characters,
    /// not bytes, and this is where the two would part.
    #[test]
    fn a_multibyte_body_verifies_as_one_field() {
        let mut call = a_call();
        call.trailing = vec![
            crate::escape::escape_markup("réponse à l'école — ✅")
                .as_str()
                .to_owned(),
        ];
        let text = render_verified(&call, &[]).expect("verifies");
        assert!(text.chars().count() < text.len(), "the body is multibyte");
        assert_eq!(read::only_block(&text, "todo").expect("reads").call, call);
    }
}

//! A value's stored form: a literal of the Prodrome's grammar, printed canonically and read back
//! by its whitelist walker, for data an event carries beyond its own fields (an inbox proposal's
//! act). A record is a constructor call whose fields are its [`Signature`]'s, in its order, so a
//! print is canonical by construction; a sum is one constructor per arm; a list is a tuple; an
//! absence is `None`; a leaf with a text form is a string read back through its own constructor.

use std::collections::BTreeSet;

use chrono::{Duration, NaiveDate};
use prodrome_core::event::{Actor, Hash};
use prodrome_core::literal::{Call, Datetime, Int, ProdromeError, Value};
use serde::de::{DeserializeOwned, IntoDeserializer as _};

/// A constructor and its fields, in printed order: one entry of a vocabulary.
pub type Signature = (&'static str, &'static [&'static str]);

/// A type with a stored literal form, read back exactly: `from_value(&x.to_value()) == Ok(x)`.
pub trait Literal: Sized {
    /// The literal.
    fn to_value(&self) -> Value;

    /// Read one back.
    ///
    /// # Errors
    /// [`ProdromeError`] naming what does not read.
    fn from_value(value: &Value) -> Result<Self, ProdromeError>;
}

/// The call of `signature` holding `values`, one per field in its order.
#[must_use]
pub fn record(signature: Signature, values: Vec<Value>) -> Value {
    let (name, fields) = signature;
    debug_assert_eq!(fields.len(), values.len(), "{name} prints every field");
    Value::call(
        name,
        fields
            .iter()
            .map(|field| (*field).to_owned())
            .zip(values)
            .collect(),
    )
}

/// The call of `body`'s constructor holding `head`'s fields first, then `values`, one per field of
/// `body` in its order: an event whose own fields are a part's, printed after the event's head.
#[must_use]
pub fn headed(head: Vec<(&'static str, Value)>, body: Signature, values: Vec<Value>) -> Value {
    let (name, fields) = body;
    debug_assert_eq!(fields.len(), values.len(), "{name} prints every field");
    Value::call(
        name,
        head.into_iter()
            .chain(fields.iter().copied().zip(values))
            .map(|(field, value)| (field.to_owned(), value))
            .collect(),
    )
}

/// The fields of a call of `signature`, read by name.
#[derive(Debug, Clone, Copy)]
pub struct Fields<'v> {
    call: &'v Call,
}

impl<'v> Fields<'v> {
    /// `value` read as a call of whichever of `signatures` it names: how a sum of records, an
    /// event schema's constructors, is read.
    ///
    /// # Errors
    /// A value that is no call, or a call of none of them.
    pub fn among(value: &'v Value, signatures: &[Signature]) -> Result<Self, ProdromeError> {
        let name = arm(value)?;
        let signature = signatures
            .iter()
            .find(|(named, _)| *named == name)
            .ok_or_else(|| ProdromeError::invalid(format!("{name} is not one of this schema's")))?;
        Self::of(value, *signature)
    }

    /// `value` read as a call of `signature`.
    ///
    /// # Errors
    /// A value that is no call of that name.
    pub fn of(value: &'v Value, signature: Signature) -> Result<Self, ProdromeError> {
        match value.as_call() {
            Some(call) if call.name == signature.0 => Ok(Self { call }),
            _ => Err(ProdromeError::invalid(format!(
                "expected {}(...), got {value:?}",
                signature.0
            ))),
        }
    }

    /// The call itself, for a reader of a part this vocabulary does not spell.
    #[must_use]
    pub fn call(&self) -> &'v Call {
        self.call
    }

    /// The constructor's name.
    #[must_use]
    pub fn name(&self) -> &'v str {
        &self.call.name
    }

    /// The field `name`, every field required.
    ///
    /// # Errors
    /// A field absent or not of `T`'s form, named.
    pub fn get<T: Literal>(&self, name: &str) -> Result<T, ProdromeError> {
        let value = self.call.field(name).ok_or_else(|| {
            ProdromeError::invalid(format!("{}(...) has no field {name}", self.call.name))
        })?;
        T::from_value(value).map_err(|failed| {
            ProdromeError::invalid(format!("{}.{name}: {failed}", self.call.name))
        })
    }
}

/// The constructor name of a sum's arm, for a `match`.
///
/// # Errors
/// A value that is no call.
pub fn arm(value: &Value) -> Result<&str, ProdromeError> {
    value
        .as_call()
        .map(|call| call.name.as_str())
        .ok_or_else(|| {
            ProdromeError::invalid(format!("expected a constructor call, got {value:?}"))
        })
}

/// The refusal for an arm no sum of `what` has.
#[must_use]
pub fn no_arm(what: &str, name: &str) -> ProdromeError {
    ProdromeError::invalid(format!("{name} is not {what}"))
}

/// A leaf whose literal is its text, read back through its own `serde` constructor, which for
/// every such leaf here parses a string.
///
/// # Errors
/// A value that is no string, or a string the leaf's constructor refuses.
pub fn text<T: DeserializeOwned>(value: &Value) -> Result<T, ProdromeError> {
    let text = value
        .as_str()
        .ok_or_else(|| ProdromeError::invalid(format!("expected a string, got {value:?}")))?;
    T::deserialize(text.to_owned().into_deserializer())
        .map_err(|failed: serde::de::value::Error| ProdromeError::invalid(failed.to_string()))
}

impl Literal for String {
    fn to_value(&self) -> Value {
        Value::str(self.as_str())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        value
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| ProdromeError::invalid(format!("expected a string, got {value:?}")))
    }
}

impl Literal for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match value {
            Value::Bool(flag) => Ok(*flag),
            other => Err(ProdromeError::invalid(format!(
                "expected a bool, got {other:?}"
            ))),
        }
    }
}

/// A natural number of any width: a literal integer is arbitrary precision, so each prints as
/// itself and reads back exactly, or is refused when it does not fit.
macro_rules! natural {
    ($($ty:ty),+) => {$(
        impl Literal for $ty {
            fn to_value(&self) -> Value {
                Value::Int(Int::new(false, &self.to_string()).expect("a natural prints as digits"))
            }

            fn from_value(value: &Value) -> Result<Self, ProdromeError> {
                match value {
                    Value::Int(int) if !int.is_negative() => int.digits().parse().ok(),
                    _ => None,
                }
                .ok_or_else(|| {
                    ProdromeError::invalid(format!(
                        "expected a natural number of at most {}, got {value:?}",
                        <$ty>::MAX
                    ))
                })
            }
        }
    )+};
}
natural!(u32, u64, usize);

impl Literal for Datetime {
    fn to_value(&self) -> Value {
        Value::Datetime(*self)
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match value {
            Value::Datetime(at) => Ok(*at),
            other => Err(ProdromeError::invalid(format!(
                "expected a datetime, got {other:?}"
            ))),
        }
    }
}

impl Literal for Actor {
    fn to_value(&self) -> Value {
        Value::str(self.as_str())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        Actor::new(String::from_value(value)?)
    }
}

impl Literal for Hash {
    fn to_value(&self) -> Value {
        Value::str(self.as_str())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        Hash::new(String::from_value(value)?)
    }
}

impl<T: Literal> Literal for Option<T> {
    fn to_value(&self) -> Value {
        self.as_ref().map_or(Value::None, Literal::to_value)
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match value {
            Value::None => Ok(None),
            other => T::from_value(other).map(Some),
        }
    }
}

impl<T: Literal> Literal for Vec<T> {
    fn to_value(&self) -> Value {
        Value::Tuple(self.iter().map(Literal::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        value
            .as_tuple()
            .ok_or_else(|| ProdromeError::invalid(format!("expected a tuple, got {value:?}")))?
            .iter()
            .map(T::from_value)
            .collect()
    }
}

/// A set prints as a tuple, sorted and each once, and reads back only from that one spelling.
impl<T: Literal + Ord> Literal for BTreeSet<T> {
    fn to_value(&self) -> Value {
        Value::Tuple(self.iter().map(Literal::to_value).collect())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        let listed = Vec::<T>::from_value(value)?;
        if listed.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ProdromeError::invalid(
                "a set is printed sorted and each member once",
            ));
        }
        Ok(listed.into_iter().collect())
    }
}

impl Literal for NaiveDate {
    fn to_value(&self) -> Value {
        Value::str(self.format("%Y-%m-%d").to_string())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        let text = String::from_value(value)?;
        NaiveDate::parse_from_str(&text, "%Y-%m-%d")
            .map_err(|failed| ProdromeError::invalid(format!("{text:?} is no day: {failed}")))
    }
}

/// A span in whole days, the only spans an act holds.
impl Literal for Duration {
    fn to_value(&self) -> Value {
        Value::int(self.num_days())
    }

    fn from_value(value: &Value) -> Result<Self, ProdromeError> {
        match value {
            Value::Int(int) => int.as_i64().and_then(Duration::try_days),
            _ => None,
        }
        .ok_or_else(|| ProdromeError::invalid(format!("expected whole days, got {value:?}")))
    }
}

/// `impl Literal` for leaves whose literal is their text: printed by `$print`, read back by
/// [`text`].
macro_rules! text_literal {
    ($($ty:ty => $print:expr),+ $(,)?) => {$(
        impl $crate::literal::Literal for $ty {
            fn to_value(&self) -> prodrome_core::literal::Value {
                let print: fn(&$ty) -> String = $print;
                prodrome_core::literal::Value::str(print(self))
            }

            fn from_value(
                value: &prodrome_core::literal::Value,
            ) -> Result<Self, prodrome_core::literal::ProdromeError> {
                $crate::literal::text(value)
            }
        }
    )+};
}
pub(crate) use text_literal;

/// An event schema's constructors are its own: none is one of the Prodrome's terms or envelopes,
/// which the walker would read first.
#[cfg(test)]
pub(crate) fn assert_own(signatures: &[Signature]) {
    use prodrome_core::literal::Vocabulary as _;
    for (name, _) in signatures {
        assert!(
            prodrome_core::term::schema::signatures()
                .signature(name)
                .is_none(),
            "{name} is a term"
        );
        assert!(
            !["Sealed", "Woven", "Genesis", "Change", "Snapshot"].contains(name),
            "{name} is an envelope"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round<T: Literal + PartialEq + std::fmt::Debug>(value: &T) {
        assert_eq!(T::from_value(&value.to_value()).as_ref(), Ok(value));
    }

    #[test]
    fn the_leaves_read_back_what_they_print() {
        round(&"it's \"quoted\"".to_owned());
        round(&true);
        round(&40u32);
        round(&u32::MAX);
        round(&u64::MAX);
        round(&usize::MAX);
        round(&Datetime::new(2026, 10, 11, 9, 0, 0, 0).expect("an instant"));
        round(&Actor::new("ada").expect("an actor"));
        round(&BTreeSet::from([3u32, 1, 2]));
        assert!(
            BTreeSet::<u32>::from_value(&Value::Tuple(vec![Value::int(2), Value::int(1)])).is_err()
        );
        assert!(
            BTreeSet::<u32>::from_value(&Value::Tuple(vec![Value::int(1), Value::int(1)])).is_err()
        );
        assert!(u32::from_value(&Value::int(i64::from(u32::MAX) + 1)).is_err());
        round(&Some(3u32));
        round(&None::<u32>);
        round(&vec!["a".to_owned(), "b".to_owned()]);
        round(&NaiveDate::from_ymd_opt(2026, 10, 5).expect("a day"));
        round(&Duration::days(21));
        assert!(u32::from_value(&Value::int(-1)).is_err());
        assert!(String::from_value(&Value::int(1)).is_err());
    }

    #[test]
    fn a_record_is_read_by_its_signature() {
        const PAIR: Signature = ("Pair", &["left", "right"]);
        let value = record(PAIR, vec![Value::int(1), Value::str("two")]);
        let fields = Fields::of(&value, PAIR).expect("a pair");
        assert_eq!(fields.get::<u32>("left"), Ok(1));
        assert_eq!(fields.get::<String>("right"), Ok("two".to_owned()));
        assert!(fields.get::<u32>("middle").is_err());
        assert!(Fields::of(&value, ("Other", &["left"])).is_err());
        assert_eq!(
            Fields::among(&value, &[("Other", &[]), PAIR]).map(|fields| fields.name()),
            Ok("Pair")
        );
        assert!(Fields::among(&value, &[("Other", &[])]).is_err());
    }
}

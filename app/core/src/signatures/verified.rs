use std::sync::LazyLock;

use prodrome_core::event::Hash;
use prodrome_core::memo::{self, Key};
use prodrome_core::sign::Signed;

use crate::table::Table;

/// How many signatures a process keeps the check of: twenty times a chain of a few thousand
/// objects' signatures. A history holding more is read correctly and checks again what was
/// dropped.
pub const KEPT_CHECKS: usize = 1 << 16;

/// The function tabulated: [`Signed::verifies`], Ed25519's strict check over the object's name.
static VERIFIES: LazyLock<Hash> =
    LazyLock::new(|| memo::name(b"prodrome sign::Signed::verifies", []));

static PROCESS: LazyLock<Verified> = LazyLock::new(|| Verified::new(KEPT_CHECKS));

/// WHETHER A SIGNATURE VERIFIES, as a table: a pure fact about one `Signed` (the object's name,
/// the key, the signature), so it is kept by the `Signed`'s content name, which covers all three,
/// and a history read again checks only the signatures it has not seen (the Prodrome's design
/// §6.4, "a tabulation by §6.2 waiting to be kept").
///
/// It means [`Signed::verifies`] and nothing else: a table dropped, or a process started cold,
/// answers the same (a law, tested).
#[derive(Debug)]
pub struct Verified(Table<bool>);

impl Verified {
    /// An empty table keeping at most `capacity` checks.
    #[must_use]
    pub fn new(capacity: usize) -> Verified {
        Verified(Table::new(capacity))
    }

    /// This process's table. It is no input of any reading, since it cannot change one, so no
    /// reader is handed it.
    #[must_use]
    pub fn process() -> &'static Verified {
        &PROCESS
    }

    /// Does `signed` verify? `name` is its content name, as a history names it.
    #[must_use]
    pub fn verifies(&self, name: &Hash, signed: &Signed) -> bool {
        self.0.through(
            Key {
                function: VERIFIES.clone(),
                argument: name.clone(),
            },
            || signed.verifies(),
        )
    }
}

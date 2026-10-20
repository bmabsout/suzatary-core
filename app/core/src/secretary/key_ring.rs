//! The keys a replica was given, as the store's pull hands them: each key's bytes in hex under
//! its name. A tab reads its lines through one, as the box reads through its key store.

use std::collections::BTreeMap;

use crate::secretary::locked_text::{KEY_BYTES, KeyName, TextKey};
use crate::secretary::words::{Gone, Words};

/// Keys by name; a name given bytes that are not its key is not held.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyRing(BTreeMap<KeyName, [u8; KEY_BYTES]>);

impl Drop for KeyRing {
    fn drop(&mut self) {
        for key in self.0.values_mut() {
            zeroize::Zeroize::zeroize(key);
        }
    }
}

impl KeyRing {
    /// The keys `given` names, each kept only when its bytes are the key its name names.
    #[must_use]
    pub fn of(given: &BTreeMap<KeyName, String>) -> Self {
        Self(
            given
                .iter()
                .filter_map(|(name, hex)| {
                    let key = TextKey::parse(name, &hex::decode(hex).ok()?)?;
                    Some((name.clone(), *key.bytes()))
                })
                .collect(),
        )
    }

    /// The key named `name`, or missing.
    #[must_use]
    pub fn key(&self, name: &KeyName) -> Words<TextKey> {
        self.0
            .get(name)
            .and_then(|bytes| TextKey::parse(name, bytes))
            .map_or_else(|| Gone::Missing.into(), Words::kept)
    }
}

/// A key as the pull spells it: its bytes in hex.
#[must_use]
pub fn spelled(key: &TextKey) -> String {
    hex::encode(key.bytes())
}

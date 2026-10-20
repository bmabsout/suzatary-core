//! What a `source` answered: the items it learned, each in a schema it names, and where its next
//! call starts. Plain data the host hands over and the laws of a sync are read from.

/// One thing a source learned: bytes in the schema the plugin names. Nothing here parses them;
/// [`super::admit`] parses each at the plugin's declared schema before anything is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// The name of the schema the bytes are in.
    pub schema: String,
    /// The bytes.
    pub bytes: Vec<u8>,
}

/// What a call learned, and the cursor the next call starts from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Synced {
    /// The items, in the order the plugin gave them.
    pub items: Vec<Item>,
    /// Opaque to the host.
    pub cursor: Vec<u8>,
}

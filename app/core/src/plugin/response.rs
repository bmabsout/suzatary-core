//! An answer, as the transport read it and as the plugin receives it.

/// One answer, whatever its status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// Headers, names lowercased.
    pub headers: Vec<(String, String)>,
    /// The body, at most what the host reads.
    pub body: Vec<u8>,
}

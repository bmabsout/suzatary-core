//! ntfy: one POST. The topic is the whole access control and the message crosses ntfy's
//! servers in the clear, which is why a push is a [`Printed`] composed by the core.

use suzatary_core::capability::Printed;

use crate::http::{Http, Req};

/// ntfy's own service, used when no `base` is configured.
pub const DEFAULT_BASE: &str = "https://ntfy.sh";

/// One ntfy POST; `false` for an unreachable relay or a non-2xx, never an error a caller must
/// handle.
pub(super) fn send(http: &dyn Http, base: &str, topic: &str, push: &Printed) -> bool {
    let req = Req::post(format!("{base}/{topic}"), push.body.as_bytes())
        .header("Title", push.title)
        .header("Click", &push.click)
        .header("Tags", push.tag)
        .header("Content-Type", "text/plain; charset=utf-8");
    http.send(req).is_ok_and(|resp| resp.ok())
}

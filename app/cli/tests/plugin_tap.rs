//! A plugin's proposal over a real server: filed as the plugin's, its card says so, and the
//! owner's tap performs it through the route every card is tapped by, the response his.

mod common;

use std::sync::Arc;

use common::Fakes;
use suzatary_core::capability::Clock as _;
use suzatary_core::plugin::{PluginName, propose};
use suzatary_core::secretary::inbox_event::Said;
use suzatary_core::secretary::{filed, inbox};
use suzatary_core::store::Prodrome as _;

fn ada() -> suzatary_core::capability::Login {
    common::login("ada@example.com")
}

const ACT: &str = "Changes(chain='life', acts=(Add(todo='duevisa', body='Renew the visa', \
                   rationale='A deadline on the calendar.', kind='todo', category='', \
                   priority=FlatPrice(percent=60), detail='', waiting_on=''),))";

#[test]
fn a_plugins_offer_is_drawn_as_its_own_and_performed_by_his_tap() {
    let whois = common::FakeWhois::new().naming(common::v4(), ada());
    let fakes = Arc::new(Fakes::new("plugin-tap").whois(whois));
    let offer = propose(
        &PluginName::parse("due").expect("a name"),
        ACT,
        "The calendar says \"Due: renew the visa\".",
        &["briefing:2026-10-05".to_owned()],
    )
    .expect("admitted");
    let filed::Offered::Filed(_, object) = filed::offer(
        fakes.conversations.conversations(),
        &fakes.inbox,
        fakes.clock.now(),
        &offer,
    )
    .expect("filed") else {
        panic!("a new act is filed as the plugin's");
    };
    let deployment = common::deployment([ada()], [(ada(), common::actor("ada"))]);
    let base = common::Server::new(Arc::clone(&fakes), deployment, Vec::new())
        .start()
        .base;

    let payload = common::reading(&base, "plugin-due", &[]);
    let card = &payload["proposals"][0];
    assert_eq!(
        card["card"]["by"],
        serde_json::json!({"kind": "plugin", "name": "due"}),
        "{payload}"
    );
    assert_eq!(card["state"], "standing");
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/todo/duevisa"))),
        404,
        "nothing is performed before the tap"
    );

    let mut tapped = common::act(
        &format!("{base}/api/secretary/conversations/plugin-due/proposals/0/tap"),
        &serde_json::json!({}),
    );
    assert_eq!(common::status(&tapped), 200);
    let after = common::body_json(&mut tapped);
    assert_eq!(after["proposals"][0]["state"], "applied", "{after}");
    assert_eq!(
        common::status(&common::get(&format!("{base}/api/todo/duevisa"))),
        200
    );

    let events = fakes.inbox.events().expect("reads");
    let responded: Vec<&str> = events
        .iter()
        .filter(|event| event.proposal == object && matches!(event.said, Said::Responded(_)))
        .map(|event| event.actor.as_str())
        .collect();
    assert_eq!(responded, ["ada"], "his tap performed it");
    let fold = inbox::fold(&fakes.inbox).expect("reads");
    assert_eq!(
        fold.get(&object).expect("held").asked.proposer,
        offer.proposer(),
        "the plugin is still who asked"
    );
}

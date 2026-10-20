/**
 * `conversation/anchor.ts` — where the transcript's box is held
 * (`docs/CHAT-PLAN-2026-09-25.md` §4.9), and `conversation/drafts.ts`, a
 * draft per conversation (§4.11).
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import anchor from "../src/chat/anchor";
import drafts from "../src/chat/drafts";
import kv from "../src/app/kv";
import ui from "../src/app/ui";
import register from "../src/algebra/register";
import { type Rng, forAll, memoryStore } from "./gen";

const event = (r: Rng): anchor.Event =>
  r.pick<anchor.Event>([
    { type: "window", fresh: r.bool(0.1) },
    { type: "asked" },
    { type: "sent", turn: r.int(20) },
    { type: "scrolled", by: r.pick(["reader", "page"] as const), at: r.pick(["bottom", "above"] as const) },
    { type: "grew" },
    { type: "pill" },
  ]);

test("law: a window moves the box only when it is the first or its turns moved", () => {
  forAll(300, (r) => {
    let was: number | null = null;
    for (let i = r.int(40); i > 0; i -= 1) {
      const total: number = r.bool(0.6) && was !== null ? was : r.int(30);
      const e = anchor.windowed(was, total);
      if (was === total) assert.equal(e, null, "a refresh that brings no turn is no event");
      else assert.deepEqual(e, { type: "window", fresh: was === null });
      was = total;
    }
  });
});

test("law: nothing that grows moves the box, and a drifted-up box is moved only by the reader", () => {
  forAll(300, (r) => {
    let t = anchor.newest;
    for (let i = r.int(40); i > 0; i -= 1) {
      const e = event(r);
      const [next, move] = anchor.step(t, e);
      if (e.type === "grew") assert.deepEqual([next, move], [t, { type: "None" }], "growing moves nothing");
      if (t.type === "Drifted" && e.type === "window" && !e.fresh) {
        assert.deepEqual(move, { type: "Pill" }, "a new answer under a drifted box is the pill, never a scroll");
      }
      if (move.type === "ToBottom") assert.ok(e.type === "pill" || e.type === "window", "only the pill or a window scrolls down");
      if (move.type === "ToTurn") assert.equal(e.type, "sent", "only a send holds a question at the top");
      t = next;
    }
  });
});

test("law: a send whose turn is known after the reader scrolled away holds nothing", () => {
  forAll(300, (r) => {
    let t = anchor.step(anchor.newest, { type: "asked" })[0];
    let away = false;
    for (let i = r.int(10); i > 0; i -= 1) {
      const e = event(r);
      if (e.type === "asked" || e.type === "sent") continue;
      away = away || (e.type === "scrolled" && e.by === "reader") || e.type === "pill" || (e.type === "window" && e.fresh);
      t = anchor.step(t, e)[0];
    }
    const [next, move] = anchor.step(t, { type: "sent", turn: 3 });
    if (away) assert.deepEqual([next, move], [t, { type: "None" }], "what the reader did since the tap stands");
    else assert.deepEqual(move, { type: "ToTurn", turn: 3 }, "a question still being asked is held at the top");
  });
});

test("law: asked again before the first is known, only the last send to be known holds a question, whatever settles between", () => {
  // What can happen while sends are in flight and the reader is away from the box.
  const between = (r: Rng): anchor.Event =>
    r.pick<anchor.Event>([{ type: "grew" }, { type: "window", fresh: false }, { type: "scrolled", by: "page", at: r.pick(["bottom", "above"] as const) }]);
  forAll(300, (r) => {
    const asks = 1 + r.int(4);
    const order: ("asked" | "sent")[] = ["asked"];
    let flying = 1;
    let left = asks - 1;
    while (flying > 0 || left > 0) {
      const ask = left > 0 && (flying === 0 || r.bool(0.5));
      order.push(ask ? "asked" : "sent");
      flying += ask ? 1 : -1;
      left -= ask ? 1 : 0;
    }
    let t: anchor.T = anchor.newest;
    let held = 0;
    order.forEach((k, i) => {
      for (let j = r.int(3); j > 0; j -= 1) t = anchor.step(t, between(r))[0];
      const [next, move] = anchor.step(t, k === "asked" ? { type: "asked" } : { type: "sent", turn: i });
      const last = order.slice(0, i + 1).filter((o) => o === "asked").length === order.slice(0, i + 1).filter((o) => o === "sent").length;
      if (k === "sent" && last) {
        assert.deepEqual(move, { type: "ToTurn", turn: i }, "the send that leaves none in flight holds its question");
        held += 1;
      } else assert.deepEqual(move.type === "ToTurn", false, "a send another is still waiting behind holds nothing");
      t = next;
    });
    assert.ok(held >= 1, "the last send to settle held one");
  });
});

test("a send holds its question at the top with room beneath, until the reader scrolls", () => {
  const [asked, move] = anchor.step(anchor.step(anchor.newest, { type: "asked" })[0], { type: "sent", turn: 7 });
  assert.deepEqual(move, { type: "ToTurn", turn: 7 });
  assert.ok(anchor.reserves(asked));
  assert.deepEqual(anchor.step(asked, { type: "window", fresh: false }), [asked, { type: "None" }], "the answer lands without moving the box");
  assert.deepEqual(anchor.step(asked, { type: "scrolled", by: "page", at: "above" })[0], asked, "the page moving the box keeps it");
  assert.deepEqual(anchor.step(asked, { type: "scrolled", by: "reader", at: "above" })[0], { type: "Drifted" }, "the reader scrolling up lets it go");
  const [back] = anchor.step(asked, { type: "scrolled", by: "reader", at: "bottom" });
  assert.deepEqual(back, anchor.newest);
  assert.ok(!anchor.reserves(back));
});

test("law: the reader's scroll settles the hold, whatever held it; the page's settles nothing", () => {
  forAll(300, (r) => {
    let t = anchor.newest;
    for (let i = r.int(20); i > 0; i -= 1) t = anchor.step(t, event(r))[0];
    assert.deepEqual(anchor.step(t, { type: "scrolled", by: "reader", at: "bottom" }), [anchor.newest, { type: "None" }]);
    assert.deepEqual(anchor.step(t, { type: "scrolled", by: "reader", at: "above" }), [{ type: "Drifted" }, { type: "None" }]);
    // A clamp as content shrinks under the box, or the browser anchoring it against content drawn above.
    for (const at of ["bottom", "above"] as const) assert.deepEqual(anchor.step(t, { type: "scrolled", by: "page", at }), [t, { type: "None" }]);
  });
});

test("scrolled up, a new window shows the pill; the pill returns to the newest", () => {
  const [drifted] = anchor.step(anchor.newest, { type: "scrolled", by: "reader", at: "above" });
  assert.deepEqual(drifted, { type: "Drifted" });
  assert.deepEqual(anchor.step(drifted, { type: "window", fresh: false })[1], { type: "Pill" });
  assert.deepEqual(anchor.step(drifted, { type: "pill" }), [anchor.newest, { type: "ToBottom" }]);
});

const tab = (store: kv.Store) => ui.over(kv.over(() => store), "tab");
const typed = (d: drafts.Draft): string => register.one(d.read(), "");
const say = (d: drafts.Draft, text: string): void => void d.write(d.seen(), text);

test("law: a draft reads back what was written, per conversation, and a blank one is not kept", () => {
  const store = memoryStore();
  const a = drafts.of(tab(store), "a");
  const b = drafts.of(tab(store), "b");
  forAll(100, (r) => {
    const text = r.string("ab \n", r.int(6));
    say(a, text);
    assert.equal(typed(a), text.trim() === "" ? "" : text);
  });
  say(b, "other");
  say(a, "  ");
  assert.equal(typed(b), "other");
  assert.equal(typed(a), "");
  assert.equal(store.size(), 1, "a blank draft is no register: its key and its history are gone");
});

test("a store that throws loses the draft and never the page", () => {
  const broken = drafts.of(ui.over(kv.over(() => {
    throw new Error("SecurityError");
  }), "tab"), "a");
  assert.equal(typed(broken), "");
  say(broken, "x");
  assert.equal(typed(broken), "", "and writing to it is a no-op, not an error");
});

test("law: a move the page asked for stands at its frame only if the reader has not touched the box since and it is held as it was", () => {
  forAll(300, (r) => {
    let held: anchor.T = anchor.newest;
    for (let i = r.int(10); i > 0; i -= 1) held = anchor.step(held, event(r))[0];
    const at = 1000 + r.int(1000);
    const asked: anchor.Asked = { at, held };
    assert.equal(anchor.stands(asked, at - 1 - r.int(1000), held), true, "input before the ask leaves the move standing");
    assert.equal(anchor.stands(asked, at + r.int(1000), held), false, "the reader's input at or after the ask wins");
    const [moved] = anchor.step(held, { type: "scrolled", by: "reader", at: "above" });
    assert.equal(anchor.stands(asked, Number.NEGATIVE_INFINITY, moved), false, "a box the reader moved is not moved back");
  });
});

test("law: the room under a held question is a window's height, and once let go it never leaves the box shorter than the reader's view", () => {
  forAll(400, (r) => {
    let held: anchor.T = anchor.newest;
    for (let i = r.int(10); i > 0; i -= 1) held = anchor.step(held, event(r))[0];
    const height = 300 + r.int(900);
    const end = r.int(5000);
    const was = r.int(height + 1);
    // The box ends where the place ends plus its room; the reader sees up to `bottom`, which that end holds.
    const bottom = Math.min(end + was, r.int(end + was + 1));
    const view = { bottom, end, height };
    const room = anchor.room(held, was, view);
    if (anchor.reserves(held)) assert.equal(room, height, "a held question keeps a window's room");
    else {
      assert.ok(room <= was, "let go, the room only shrinks");
      assert.ok(end + room >= bottom, "and the box still reaches as far as the reader sees, so nothing pulls them down");
      assert.equal(room, Math.max(0, Math.min(was, bottom - end)), "and it keeps only what the reader still sees");
    }
  });
});

test("a reader scrolled a screen above the place lets the whole room go; one still looking into it keeps what they see", () => {
  const drifted: anchor.T = { type: "Drifted" };
  assert.equal(anchor.room(drifted, 800, { bottom: 1000, end: 1500, height: 800 }), 0);
  assert.equal(anchor.room(drifted, 800, { bottom: 1900, end: 1500, height: 800 }), 400);
  assert.equal(anchor.room({ type: "Question", turn: 3 }, 0, { bottom: 1900, end: 1500, height: 800 }), 800);
});

test("the reader's place is read against what the box shows, never against the room", () => {
  // 1266 px above the end of a box holding a window of room, 422 px above what it shows: within a screen of it.
  const box = { top: 1718, scrollHeight: 4206, room: 844, height: 844 };
  assert.equal(anchor.placed(box), "bottom");
  assert.equal(anchor.placed({ ...box, scrollHeight: box.scrollHeight - box.room, room: 0 }), "bottom", "the same once the room goes");
  assert.equal(anchor.placed({ ...box, top: box.top - 423 }), "above", "past a screen above what it shows");
});

/**
 * A box as the transcript moves it: `content` is everything above the room, the room is the last thing in it, and the
 * reader sees `height` from `top`. Every scroll is heard as the transcript hears it, by a mover the page cannot know for
 * sure (`by`), and the room is set again after every step, as `move` does.
 */
type Box = { content: number; room: number; top: number; readonly height: number; held: anchor.T };

const settle = (b: Box, e: anchor.Event): anchor.Move => {
  const [next, move] = anchor.step(b.held, e);
  b.held = next;
  b.room = anchor.room(b.held, b.room, { bottom: b.top + b.height, end: b.content, height: b.height });
  assert.ok(b.top <= Math.max(0, b.content + b.room - b.height), "setting the room never cuts the box under the reader");
  return move;
};
const placed = (b: Box): anchor.Place => anchor.placed({ top: b.top, scrollHeight: b.content + b.room, room: b.room, height: b.height });
const scrolled = (b: Box, by: anchor.Mover): anchor.Move => settle(b, { type: "scrolled", by, at: placed(b) });
const scrollTo = (b: Box, top: number): void => void (b.top = Math.max(0, Math.min(top, b.content + b.room - b.height)));

test("law: a scroll that leaves the reader where they were changes no hold that agrees with their place, whoever it is counted to, and a drifted box reads as above", () => {
  forAll(500, (r) => {
    const height = 400 + r.int(800);
    const b: Box = { content: height + r.int(4000), room: 0, top: 0, height, held: anchor.newest };
    scrollTo(b, b.content);
    settle(b, { type: "asked" });
    // The question just sent is held at the top of the box, with a window of room beneath it.
    if (settle(b, { type: "sent", turn: 3 }).type === "ToTurn") scrollTo(b, b.content - r.int(height));
    for (let i = r.int(30); i > 0; i -= 1) {
      const was = b.held;
      switch (r.int(5)) {
        case 0:
          // The reader's own wheel, touch or key.
          scrollTo(b, r.int(b.content + b.room));
          scrolled(b, "reader");
          break;
        case 1: {
          // Content drawn above the reader: the browser anchors the box, so the reader sees what they saw. Heard within
          // a moment of their input, it is counted as theirs.
          const before = placed(b);
          const drawn = 1 + r.int(2000);
          b.content += drawn;
          b.top += drawn;
          assert.equal(placed(b), before, "the reader is where they were");
          scrolled(b, r.pick(["reader", "page"] as const));
          if ((was.type === "Newest" && before === "bottom") || was.type === "Drifted") assert.deepEqual(b.held, was, "so a hold that agreed with their place is as it was");
          break;
        }
        case 2:
          b.content += 1 + r.int(800);
          settle(b, { type: "grew" });
          break;
        case 3:
          b.content += 1 + r.int(800);
          if (settle(b, { type: "window", fresh: false }).type === "ToBottom") {
            scrollTo(b, b.content + b.room);
            scrolled(b, r.pick(["reader", "page"] as const));
          }
          break;
        case 4:
          if (settle(b, { type: "pill" }).type === "ToBottom") {
            scrollTo(b, b.content + b.room);
            scrolled(b, r.pick(["reader", "page"] as const));
          }
          break;
      }
      if (b.held.type === "Drifted") assert.equal(placed(b), "above", "drifted is a place above");
    }
  });
});

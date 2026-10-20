/** The cycle a recurring price carries, read back off the term for the edit form. */

import type { SpecNode } from "../wire/generated/wire";
import d from "../wire/decoder";
import lo from "../algebra/lo";
import payload from "../wire/generated/payload";
import price from "./price";
import term from "./term";
import time from "../app/time";

export namespace recur {
  const isSpecNode = d.is(payload.SpecNode)
  type Point = { at: string; value: number }
  const Point = d.obj<Point>({ at: d.str, value: d.num })
  const Points = d.tuple(Point, Point)

  // `Cycle::ramp`'s two-point curve read back: `days` apart, `from`/`to` their values.
  const twoPoints = (t: SpecNode): price.Cycle | null => {
    const points = Points(t["points"]);
    return lo.withDefault(
      lo.map(points, ([a, b]) => ({
        days: Math.round(((time.ms(b.at) ?? NaN) - (time.ms(a.at) ?? NaN)) / 86_400_000),
        from: a.value,
        to: b.value,
      })),
      null,
    );
  };

  const own = (t: SpecNode): price.Form | null => {
    const inner = t["term"];
    const cycle = (t.kind === "recur" || t.kind === "periodic") && isSpecNode(inner) ? twoPoints(inner) : null;
    return cycle === null ? null : t.kind === "recur" ? price.recur(cycle) : price.periodic(cycle);
  };

  /** The cycle a `Recur` or `Periodic` term carries, as a form the edit panel opens with; `null` for a price with none. */
  export const priceOf = (t: SpecNode): price.Form | null => term.find(own, t);
}

export default recur;

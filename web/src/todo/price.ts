// A todo's price, authored by the forms: a sum carrying only its own kind's fields, flat, a deadline, or a cycle after a pass or on the calendar.

import type { Price } from "../wire/generated/wire";
import parsed from "../algebra/parsed";
import fulfillment from "./fulfillment";

export namespace price {
  /** `todo::Cycle`; `from` and `to` are ratios, like `flat`. */
  export interface Cycle {
    readonly days: number;
    readonly from: number;
    readonly to: number;
  }

  export type Form =
    | { readonly type: "Flat"; readonly flat: number }
    | { readonly type: "Deadline"; readonly deadline: string; readonly leadUpDays: number }
    | { readonly type: "Recur"; readonly cycle: Cycle }
    | { readonly type: "Periodic"; readonly cycle: Cycle };

  export type Kind = Form["type"];

  export const KINDS: readonly { readonly value: Kind; readonly label: string }[] = [
    { value: "Flat", label: "Flat" },
    { value: "Deadline", label: "Deadline" },
    { value: "Recur", label: "After a pass" },
    { value: "Periodic", label: "On the calendar" },
  ];

  // The middle of the scale: a real number the author moves, never one that looks unset.
  export const DEFAULT_FLAT = 0.5;
  export const DEFAULT_LEAD_UP_DAYS = 14;
  export const DEADLINE_START_PERCENT = 60;
  export const DEADLINE_END_PERCENT = 5;
  export const DEFAULT_CYCLE: Cycle = { days: 30, from: 0.7, to: 0.3 };

  export const flat = (flat: number): Form => ({ type: "Flat", flat });
  export const deadline = (deadline: string, leadUpDays: number): Form => ({
    type: "Deadline",
    deadline,
    leadUpDays,
  });
  export const recur = (cycle: Cycle): Form => ({ type: "Recur", cycle });
  export const periodic = (cycle: Cycle): Form => ({ type: "Periodic", cycle });

  export const EMPTY: Form = flat(DEFAULT_FLAT);

  // The two cycling kinds share a `Cycle`, so one typed under either survives a tap onto the other; the rest start fresh.
  export function withKind(form: Form, kind: Kind): Form {
    if (form.type === kind) return form;
    switch (kind) {
      case "Flat":
        return flat(DEFAULT_FLAT);
      case "Deadline":
        return deadline("", DEFAULT_LEAD_UP_DAYS);
      case "Recur":
        return recur(form.type === "Periodic" ? form.cycle : DEFAULT_CYCLE);
      case "Periodic":
        return periodic(form.type === "Recur" ? form.cycle : DEFAULT_CYCLE);
    }
  }

  /** The cycle a cycling form carries, else the default: what the cycle's hidden rows show. */
  export function cycleOf(form: Form): Cycle {
    return form.type === "Recur" || form.type === "Periodic" ? form.cycle : DEFAULT_CYCLE;
  }

  export function withCycle(form: Form, cycle: Cycle): Form {
    if (form.type === "Recur") return recur(cycle);
    if (form.type === "Periodic") return periodic(cycle);
    return form;
  }

  export const wholePercent = fulfillment.whole;

  export const percentOf = (value: number): number => Math.round(wholePercent(value) * 100);

  const DATE = /^\d{4}-\d{2}-\d{2}$/;

  export function of(form: Form): parsed.T<Price> {
    switch (form.type) {
      case "Flat":
        return parsed.ok({ how: "flat", percent: percentOf(form.flat) });
      case "Deadline": {
        if (!DATE.test(form.deadline)) return parsed.refused("Pick a deadline date.");
        if (!Number.isInteger(form.leadUpDays) || form.leadUpDays < 1 || form.leadUpDays > 365) {
          return parsed.refused("The lead-up is a whole number of days, 1 to 365.");
        }
        return parsed.ok({
          how: "deadline",
          end_date: form.deadline,
          start_percent: DEADLINE_START_PERCENT,
          end_percent: DEADLINE_END_PERCENT,
          lead_up_weeks: null,
          lead_up_days: form.leadUpDays,
          start_date: null,
        });
      }
      case "Recur":
      case "Periodic": {
        const { days, from, to } = form.cycle;
        if (!Number.isInteger(days) || days < 1 || days > 3650) {
          return parsed.refused("The cycle runs 1 to 3650 days.");
        }
        const cycle = { days, from_percent: percentOf(from), to_percent: percentOf(to) };
        return parsed.ok({ how: form.type === "Recur" ? "recur" : "periodic", ...cycle });
      }
    }
  }
}

export default price;

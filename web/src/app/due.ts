// When the fold on screen next moves: the core's next change, a dated row entering or leaving the fortnight, or the
// owner's midnight for what is printed to the day.

import clock from "../algebra/clock"
import { datedNext } from "../todo/todos"
import onscreen from "./store"
import type time from "./time"

export namespace due {
  export const of = (f: onscreen.Fold, now: number, zone: time.Zone): clock.Next => {
    if (f.type !== "Shown") return clock.never
    const dated = datedNext(f.entries, f.at)
    return clock.sooner(clock.of(onscreen.next(f)), dated === null ? clock.never : clock.at(dated), clock.nextDay(zone, now))
  }
}
export default due

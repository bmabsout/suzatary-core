// What a card names its sources by, read off the fold on screen: each todo's title, read again whenever that fold moves
// and the same map while no title did, so a link opened before the todos arrived names its sources once they have.

import reading from "../algebra/reading"
import onscreen, { store } from "../app/store"
import spoken from "./spoken"

export namespace onScreen {
  export const titles: reading.Reading<spoken.Titles> = reading.settled(
    reading.map(reading.held(() => onscreen.entries(store.state()), heard => store.subscribe(() => heard())), spoken.titles),
    spoken.same,
  )
}
export default onScreen

// The core in its own thread: one module, the chains the tab holds read into it once per change, and every question
// about them answered here (`questions.serve`), so a page change never waits on the core's reading of a chain.

import { loadCore } from "./core"
import questions from "./questions"

// The DOM's types name `self` a window; a dedicated worker's scope answers on the same port it listens on.
const scope: questions.Port<questions.Asked, questions.Answered> = self
questions.listen(scope, loadCore().then(questions.serve))

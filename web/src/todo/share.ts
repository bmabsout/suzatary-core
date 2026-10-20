// What another app shared into Suzatary, made into a new todo's body. Its own module so the router can read it
// without importing the add sheet's form.

export namespace share {
  // A share is two fields: what was shared (often a URL) and the page's own name. One body, and a title the
  // text already holds is not said twice. The sheet has no detail field, so a URL stays in the body.
  export const body = (text: string, title: string): string => {
    const said = text.trim()
    const name = title.trim()
    if (name === "" || said.includes(name)) return said
    if (said === "") return name
    return `${name}\n${said}`
  }
}
export default share

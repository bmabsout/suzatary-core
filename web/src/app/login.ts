// Who `/api/me` says this tab is, parsed once where it arrives: a login that writes, a login that only reads, or this
// box's own node, which the gate names with no login and never lets write.

export namespace login {
  export type Met = { type: "Box" } | { type: "Reads"; login: string } | { type: "Writes"; login: string; actor: string }

  export const of = (me: { readonly login: string; readonly actor: string | null }): Met =>
    me.login === "" ? { type: "Box" } : me.actor === null ? { type: "Reads", login: me.login } : { type: "Writes", login: me.login, actor: me.actor }
}
export default login

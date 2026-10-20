# Todos, subtodos and groups are one tree of terms — 2026-09-25

The owner, 2026-09-24/25: one-line rows that grow when opened, so many todos fit;
a group-by that composes fulfillments the way the document did, home vs work
vs the rest by default; "not 1 level … sub sub sub todos should be possible";
"Edward Kmett level". Later: the TypeScript to scanman's style. Chains:
`todotree`, `fulfillmentexpr` (project chain `suzatary`).

## 1. The algebra

- **A reference is a variable.** The Prodrome's term is already a base functor
  with a fixed point (`TermF<A>`, `map`, every recursion a fold). Add one leaf,
  `Ref { todo }`: the fulfillment of another todo. A term with references is
  `Free TermF TodoId`; `fulfillment` is defined only on closed terms.
- **Linking is bind.** `link(term, specs) -> Result<Term, Cycle>` substitutes
  each `Ref(x)` with `x`'s own history (its flattened `Piecewise`), recursively.
  The reference graph must be a DAG; a cycle is refused when the spec that
  would close it is authored, and again at link time (total, never loops).
  Law: `fulfillment(link(Ref(x)), t) == fulfillment(x, t)` for every `t`.
- **A subtodo is a reference, not a field.** A parent's spec is
  `Conj[own, Ref(c1), …, Ref(cn)]`. Adding a subtodo authors the child and
  revises the parent's spec; nothing is stored twice and no frozen field
  changes. The hierarchy is the reference DAG, derived, like every view.
- **A group is a term the view builds.** A classifier maps a todo to a path
  (`category`, then its ancestry). Paths build a trie (the free structure on
  paths); classifiers compose by path concatenation, a monoid. The trie's
  catamorphism builds a TERM per node: a leaf is `Ref(todo)`, an inner node is
  `Conj` of its children (p = −4, the list's one conjunction). The core links
  and evaluates it. No view computes a number; it builds a term and asks.
- **Consequence:** a group's value, a parent's value and a todo's value are
  the same kind of thing, with the same graph, explanation and time machine.

## 2. What each layer owns

- **Prodrome (public crate, a MINOR release):** `TermF::Ref`, its literal
  `Ref(todo='…')`, `link`, the cycle check, SPEC §7 text and conformance
  vectors; `prodrome-wasm` exports `link`.
- **Suzatary core:** `view::tree(entries, classifier) -> Tree<Row>`, the
  classifiers (category, ancestry), the terms the trie builds, and authoring
  "add a subtodo" (child plus the parent's spec revision). The existing flat
  checklist (`SubTodo`, no id) stays readable and unchanged.
- **Wire and server:** `GET /api/<chain>/tree?by=category` answers the tree
  with each node's value, its term's print, and its children.
- **Web, in scanman's style:** `namespace tree` (the `Tree<A>` type, `map`,
  `cata`, `visible(expanded)`), `namespace row` (a node as one line: title,
  value, expander; opening shows the card), pure and node-tested, drawn
  through the keyed `reconcile`. Keyboard: j/k move, l/Enter open, h close.
  The offline fold builds the same tree through the wasm core.

## 3. The series

1. prodrome: `Ref`, `link`, the cycle refusal, laws (link of a ref is its
   target; closed terms unchanged; a cycle never evaluates), SPEC, vectors,
   wasm. Released as a tag; Suzatary's rev and flake input move together.
2. core: classifiers, `trie`, `view::tree`, the per-node terms, with the law
   "a group of one is its todo" and "grouping twice by the same key is
   grouping once".
3. core + cli: author a subtodo (child and parent revision, one act); the
   agent's and the secretary's `todo_add` take `parent`.
4. wire + route: `/api/<chain>/tree`.
5. web: `tree` and `row` namespaces, the list as rows, category grouping by
   default, the expand-to-card interaction; e2e.
6. docs: WEB-SPEC laws, SECRETARY.md and OPERATOR.md on `parent`.

## 4. Not now

Recurrence (`recurringfpl`: a tending event and a term reading it) is the
same crate's next release and its own plan. Re-rendering the typst document
as the same tree waits until the web tree has settled.

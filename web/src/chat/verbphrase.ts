/**
 * A verb's wire name spelled for a person; the prose lives here so the core stays ignorant of it.
 * Not exhaustive on purpose: an unlisted verb reads as its own name, never as nothing.
 */

/** Phrased by what is happening, so verbs that do the same thing share a phrase. */
const PHRASES: Readonly<Record<string, string>> = {
  search: "searching mail",
  show: "reading a message",
  todo_show: "reading todos",
  list_open_todos: "reading todos",
  briefing_delta: "reading what changed",
  context: "reading notes",
  propose_email: "drafting an email",
  repos_list: "reading repositories",
  repo_tree: "reading a repository",
  repo_read: "reading a repository",
  repo_grep: "searching a repository",
  propose_patch: "drafting a change",
  propose_coding_session: "drafting a coding session",
  drive_list: "reading Drive",
  drive_read: "reading a document",
  junk_digest: "reading Junk",
  junk_show: "reading a Junk message",
};

export function verbPhrase(verb: string): string {
  return PHRASES[verb] ?? verb;
}

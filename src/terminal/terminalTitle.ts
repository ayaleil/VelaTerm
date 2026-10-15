import type { SessionKind } from "../types";

/** Session kinds whose terminal title (OSC 0/2) renames the session. Plain shells are excluded because they
 * rewrite the title on every prompt and would clobber names like "dev server". */
const TITLE_RENAMES_KINDS: SessionKind[] = ["claude"];

/** Titles an agent sets before it has a topic; they are not a rename. */
const DEFAULT_TITLES = new Set(["Claude Code"]);

const MAX_NAME_LENGTH = 80;

/**
 * Turns a terminal title into a session name, or null when the title should not rename the session.
 * Claude prefixes its title with a status glyph (✳ idle, braille spinner while working); it is stripped so
 * the name stays stable while the status changes.
 */
export function sessionNameFromTitle(kind: SessionKind, title: string): string | null {
  if (!TITLE_RENAMES_KINDS.includes(kind)) return null;
  const name = title.replace(/^[^\p{L}\p{N}]+/u, "").trim().slice(0, MAX_NAME_LENGTH);
  if (!name || DEFAULT_TITLES.has(name)) return null;
  return name;
}

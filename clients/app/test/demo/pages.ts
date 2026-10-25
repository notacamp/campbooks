// Fictional pages for tests and stories (never a real person): Ana Duarte's
// notes at Acme Studio, as the pages API returns them.
import * as Y from "yjs";

import type { Schema } from "~/lib/api";

import { toBase64 } from "../../src/modules/pages/editor/bytes";

import { ANA, RUI } from "./workspaces";

type Summary = Schema<"PageSummary">;
type View = Schema<"PageView">;

const page = (n: number, over: Partial<Summary>): Summary => ({
  id: `0192f1a0-0000-7000-8000-0000000009${n.toString(16).padStart(2, "0")}`,
  parent: null,
  place: "workspace",
  position: "a0",
  title: "",
  icon: null,
  has_children: false,
  role: "manager",
  created_by: ANA.user_id,
  edited_at: "2026-10-12T09:00:00Z",
  trashed_at: null,
  meta_revision: 1,
  ...over,
});

/** The team handbook, with two pages under it. */
export const HANDBOOK = page(1, {
  title: "Team handbook",
  icon: "document",
  position: "a0",
  has_children: true,
});
export const ONBOARDING = page(2, {
  title: "Onboarding checklist",
  icon: "tasks",
  parent: HANDBOOK.id,
  position: "a0",
});
export const HOLIDAYS = page(3, {
  title: "Holiday calendar",
  icon: "schedule",
  parent: HANDBOOK.id,
  position: "a1",
});
/** A page only Ana sees. */
export const IDEAS = page(4, {
  title: "Ideas for the Acme Studio kickoff",
  icon: "star",
  place: "private",
  position: "a0",
});
/** Rui's private page, shared with Ana to read. */
export const BUDGET = page(5, {
  title: "Budget draft",
  icon: "money",
  place: "private",
  created_by: RUI.user_id,
  role: "reader",
  position: "a0",
});
export const OLD_NOTES = page(6, {
  title: "Old meeting notes",
  place: "private",
  position: "a1",
  trashed_at: "2026-10-10T09:00:00Z",
});

export const PAGES: readonly Summary[] = [HANDBOOK, ONBOARDING, HOLIDAYS, IDEAS, BUDGET];

/** A document of two paragraphs, as the server stores it. */
function demoDocument(...lines: string[]): { update: string; state_vector: string } {
  const doc = new Y.Doc();
  const fragment = doc.getXmlFragment("default");
  for (const line of lines) {
    const paragraph = new Y.XmlElement("paragraph");
    paragraph.insert(0, [new Y.XmlText(line)]);
    fragment.push([paragraph]);
  }
  return {
    update: toBase64(Y.encodeStateAsUpdate(doc)),
    state_vector: toBase64(Y.encodeStateVector(doc)),
  };
}

/** The page as `pages.get` returns it. */
export function viewOf(summary: Summary, lines: string[]): View {
  return {
    ...summary,
    created_at: "2026-10-01T09:00:00Z",
    edited_by: summary.created_by,
    revision: 1,
    ancestors: [],
    grants: [],
    document: demoDocument(...lines),
  };
}

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
  kind: "page",
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

/** A member of the demo workspace who is not in the other demos. */
export const JONAS = {
  user_id: "0192f1a0-0000-7000-8000-0000000000c1",
  display_name: "Jonas Weber",
  email: "jonas@example.com",
  role: "member",
  joined_at: "2026-09-12T09:00:00Z",
  you: false,
} as const;

/** A task the viewer cannot open: no read answers for it. */
const SECRET_TASK_ID = "0192f1a0-0000-7000-8000-00000000dead";

/** The document, as the server stores it. */
function documentOf(doc: Y.Doc): { update: string; state_vector: string } {
  return {
    update: toBase64(Y.encodeStateAsUpdate(doc)),
    state_vector: toBase64(Y.encodeStateVector(doc)),
  };
}

function block(
  name: string,
  attributes: Record<string, string | number | boolean>,
  ...inline: (string | Y.XmlElement)[]
) {
  const element = new Y.XmlElement(name);
  for (const [key, value] of Object.entries(attributes)) {
    element.setAttribute(key, value as string);
  }
  if (inline.length > 0) {
    element.insert(
      0,
      inline.map((part) => (typeof part === "string" ? new Y.XmlText(part) : part)),
    );
  }
  return element;
}

const mention = (type: string, id: string) => block("mention", { ref_type: type, ref_id: id });
const wrap = (
  name: string,
  attributes: Record<string, string | number | boolean>,
  ...inside: Y.XmlElement[]
) => {
  const element = new Y.XmlElement(name);
  for (const [key, value] of Object.entries(attributes)) {
    element.setAttribute(key, value as string);
  }
  element.insert(0, inside);
  return element;
};

/** Paragraphs of plain text. */
export function plainDoc(...lines: string[]): Y.Doc {
  const doc = new Y.Doc();
  doc.getXmlFragment("default").push(lines.map((line) => block("paragraph", {}, line)));
  return doc;
}

/**
 * A page that shows the editor off: headings, a paragraph, three to-dos
 * (one done), a callout, a list, and mentions of a person, a thread, a task
 * and a task the viewer cannot open.
 */
export function richDoc(threadId: string, taskId: string): Y.Doc {
  const doc = new Y.Doc();
  const todo = (text: string, checked: boolean) =>
    wrap("taskItem", { checked }, block("paragraph", {}, text));
  const item = (...inline: (string | Y.XmlElement)[]) =>
    wrap("listItem", {}, block("paragraph", {}, ...inline));
  doc
    .getXmlFragment("default")
    .push([
      block("heading", { level: 1 }, "How we work"),
      block("paragraph", {}, "Welcome to Acme Studio. This is how we work together."),
      block(
        "paragraph",
        {},
        "Questions go to ",
        mention("workspaces.member", JONAS.user_id),
        " first.",
      ),
      block("heading", { level: 2 }, "This week"),
      wrap(
        "taskList",
        {},
        todo("Book the venue", true),
        todo("Send the agenda", false),
        todo("Confirm the budget", false),
      ),
      wrap(
        "callout",
        { tone: "info" },
        block("paragraph", {}, "The kickoff is on Thursday at 10:00."),
      ),
      block("heading", { level: 2 }, "Linked from here"),
      wrap(
        "bulletList",
        {},
        item("Email: ", mention("conversations.conversation", threadId)),
        item("Task: ", mention("tasks.task", taskId)),
        item("Budget notes: ", mention("tasks.task", SECRET_TASK_ID)),
      ),
      block("paragraph", {}, "Notes from the kickoff go below."),
    ]);
  return doc;
}

/**
 * A page with every block of the schema, one after another, as people write
 * them: the page the block spacing is checked against.
 */
export function everyBlockDoc(threadId: string, taskId: string): Y.Doc {
  const doc = new Y.Doc();
  const para = (...inline: (string | Y.XmlElement)[]) => block("paragraph", {}, ...inline);
  const item = (...inline: (string | Y.XmlElement)[]) => wrap("listItem", {}, para(...inline));
  const todo = (text: string, checked: boolean, ...inside: Y.XmlElement[]) =>
    wrap("taskItem", { checked }, para(text), ...inside);
  doc
    .getXmlFragment("default")
    .push([
      para(
        "Every new client gets the same first week. This page is the checklist, and Scout ticks what it can.",
      ),
      block("heading", { level: 1 }, "Before the kickoff"),
      para("Three things have to be true before anyone books a room."),
      para("Questions go to ", mention("workspaces.member", JONAS.user_id), " first."),
      block("heading", { level: 2 }, "What we send"),
      wrap(
        "bulletList",
        {},
        item("The welcome pack"),
        wrap(
          "listItem",
          {},
          para("The contract, with"),
          wrap("bulletList", {}, item("the rate card"), item("the payment terms")),
        ),
        item("The first invoice"),
      ),
      block("heading", { level: 3 }, "In this order"),
      wrap(
        "orderedList",
        {},
        item("Contract signed"),
        item("Deposit received"),
        item("Kickoff booked"),
      ),
      block("heading", { level: 2 }, "To do"),
      wrap(
        "taskList",
        {},
        todo("Book the venue", true),
        todo(
          "Send the agenda",
          false,
          wrap("taskList", {}, todo("Draft it", true), todo("Ask Jonas to read it", false)),
        ),
        todo("Confirm the budget", false),
      ),
      wrap("callout", { tone: "info" }, para("The kickoff is on Thursday at 10:00.")),
      wrap(
        "callout",
        { tone: "warning" },
        para("Never send the contract before the rate card is signed off."),
        para("Legal reads it first."),
      ),
      wrap("blockquote", {}, para("Keep it short, and send it the same day.")),
      block("horizontalRule", {}),
      block("heading", { level: 2 }, "Linked from here"),
      para("The thread with the client: ", mention("conversations.conversation", threadId)),
      block("embed", { ref_type: "tasks.task", ref_id: taskId }),
      block("pageLink", { page: "0192f1a0-0000-7000-8000-000000000903" }),
      block("file", { blob: "brand-guide", name: "brand-guide.pdf", size: 2_400_000 }),
      wrap(
        "toggle",
        { open: true },
        para("How we price"),
        para("Day rate, billed weekly. Expenses at cost."),
      ),
      wrap("toggle", { open: false }, para("Old rates"), para("Hidden until opened.")),
      block("heading", { level: 3 }, "The invoice line"),
      block("codeBlock", {}, "Kickoff workshop · 1 day · €1,200"),
      para("Notes from the kickoff go below."),
    ]);
  return doc;
}

/** The page as `pages.get` returns it, with `doc` as its body. */
export function viewOfDoc(summary: Summary, doc: Y.Doc, extra: Partial<View> = {}): View {
  return {
    ...summary,
    created_at: "2026-10-01T09:00:00Z",
    edited_by: summary.created_by,
    revision: 1,
    ancestors: [],
    grants: [],
    document: documentOf(doc),
    ...extra,
  };
}

// --- Notion (campbooks-cloud#376) -------------------------------------------

/** Notion's pages, as the connection lists them: Ana shared a roadmap with two sub-pages. */
export const NOTION_ROADMAP = "0192f1a0-0000-7000-8000-0000000007a1";
export const NOTION_OKRS = "0192f1a0-0000-7000-8000-0000000007a2";
const NOTION_RETRO = "0192f1a0-0000-7000-8000-0000000007a3";
export const NOTION_NOTES = "0192f1a0-0000-7000-8000-0000000007a4";
const NOTION_HIRING = "0192f1a0-0000-7000-8000-0000000007a5";

type NotionPage = Schema<"NotionPage">;

const notionPage = (id: string, title: string, over: Partial<NotionPage> = {}): NotionPage => ({
  notion_page: id,
  title,
  edited_at: "2026-10-25T09:00:00Z",
  parent: null,
  sub_pages: 0,
  linked: false,
  ...over,
});

export const NOTION_PAGES: readonly NotionPage[] = [
  notionPage(NOTION_ROADMAP, "Product roadmap", { sub_pages: 2 }),
  notionPage(NOTION_OKRS, "Q4 OKRs", { parent: NOTION_ROADMAP }),
  notionPage(NOTION_RETRO, "Launch retro", { parent: NOTION_ROADMAP }),
  notionPage(NOTION_NOTES, "Meeting notes"),
  notionPage(NOTION_HIRING, "Hiring plan", { linked: true }),
];

/** The link of a page to its Notion page, syncing, last moved a minute ago. */
export function notionLink(
  page: string,
  notionPageId: string,
  over: Partial<Schema<"NotionLink">> = {},
): Schema<"NotionLink"> {
  return {
    page,
    connection: "0192f1a0-0000-7000-8000-0000000000c9",
    notion_page: notionPageId,
    state: "syncing",
    stopped_reason: null,
    owner: ANA.user_id,
    conflicts: 0,
    last_inbound_at: "2026-10-25T09:00:00Z",
    last_outbound_at: null,
    ...over,
  };
}

/** The stable id of the paragraph the demo conflict is about. */
export const CONFLICT_BLOCK = "0192f1a0-0000-7000-8000-0000000008b1";

/**
 * A page synced with Notion: a paragraph with a conflict beneath it (the
 * launch date, changed on both sides), a block kept in Notion, and a
 * paragraph nobody disagrees about. Blocks carry their stable ids.
 */
export function notionDoc(): Y.Doc {
  const doc = new Y.Doc();
  const withId = (name: string, bid: string, ...inline: string[]) => {
    const element = block(name, { bid }, ...inline);
    return element;
  };
  doc.getXmlFragment("default").push([
    withId("heading", "0192f1a0-0000-7000-8000-0000000008a0", "Product roadmap"),
    withId("paragraph", CONFLICT_BLOCK, "We launch on the 14th."),
    wrap(
      "conflict",
      {
        of: CONFLICT_BLOCK,
        hash: "ab".repeat(32),
        edited: "2026-10-25T09:00:00.000Z",
        removed: false,
      },
      block("paragraph", {}, "We launch on the 21st."),
    ),
    block("external", { bid: "0192f1a0-0000-7000-8000-0000000008b2", kind: "column_list" }),
    withId("paragraph", "0192f1a0-0000-7000-8000-0000000008b3", "Rui owns the launch checklist."),
  ]);
  return doc;
}

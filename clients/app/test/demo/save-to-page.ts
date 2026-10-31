// A pretend pages server for "Save to page" (campbooks-cloud#364), in tests
// and stories: Ana saves Jonas Weber's "Kickoff agenda" thread to
// "Client onboarding" or "Acme Studio". Demo data only (never a real person).
// Routes are neutral (`(body) => answer`); `forRequests` adapts them to the
// test mock, whose handlers also get the request.
import * as Y from "yjs";

import type { Schema } from "~/lib/api";

import { fromBase64, toBase64 } from "../../src/modules/pages/editor/bytes";
import type { SaveThread } from "../../src/modules/pages";

import { W } from "./conversations";
import { ACME, CLIENT_ONBOARDING, HANDBOOK, IDEAS, JONAS, plainDoc, viewOfDoc } from "./pages";
import { ANA, RUI, SOFIA } from "./workspaces";

export interface Answer {
  status?: number;
  body?: unknown;
}
export type Handler = (body: unknown, url: URL) => Answer;

/** The thread Ana saves. */
export const KICKOFF_ID = "0192f1a0-0000-7000-8000-00000000a001";
export const KICKOFF_THREAD: SaveThread = {
  id: KICKOFF_ID,
  subject: "Kickoff agenda",
  messages: [
    {
      id: "0192f1a0-0000-7000-8000-00000000b001",
      from: "Jonas Weber",
      sentAt: "2026-10-21T09:12:00Z",
      text: "Hi Ana, three things for Thursday: the brand files we already have, who signs off on copy on our side, and whether the first invoice can wait until November.",
      attachments: [
        { id: "0192f1a0-0000-7000-8000-00000000c001", name: "brand-guide.pdf", size: 2_100_000 },
        { id: "0192f1a0-0000-7000-8000-00000000c002", name: "logo-pack.zip", size: 8_400_000 },
      ],
    },
    {
      id: "0192f1a0-0000-7000-8000-00000000b002",
      from: "Ana Duarte",
      sentAt: "2026-10-21T10:30:00Z",
      text: "Thanks Jonas. I will bring the contract.",
      attachments: [],
    },
  ],
};

/** The same thread with a file over 20 MiB. */
export const HEAVY_THREAD: SaveThread = {
  ...KICKOFF_THREAD,
  messages: [
    ...KICKOFF_THREAD.messages.map((message, index) =>
      index === 0
        ? {
            ...message,
            attachments: [
              { id: "0192f1a0-0000-7000-8000-00000000c003", name: "footage.mov", size: 48_000_000 },
            ],
          }
        : message,
    ),
  ],
};

const MEMBERS = [ANA, SOFIA, RUI, JONAS];
const WRITABLE = [CLIENT_ONBOARDING, ACME, HANDBOOK, IDEAS];

/** Who reads each page, as `pages.get_sharing` answers. */
const SHARING: Record<string, Schema<"PageSharing">> = {
  [CLIENT_ONBOARDING.id]: { everyone: true, notion: false, place: "workspace", readers: [] },
  [ACME.id]: {
    everyone: false,
    notion: false,
    place: "private",
    readers: [ANA.user_id, RUI.user_id],
  },
  [HANDBOOK.id]: { everyone: true, notion: true, place: "workspace", readers: [] },
  [IDEAS.id]: { everyone: false, notion: false, place: "private", readers: [ANA.user_id] },
};

export interface SaveServerOptions {
  /** Pages the thread already appears on (shown first). Default: Client onboarding. */
  linked?: readonly string[];
  /** `pages.attach_from_message` answers this refusal. */
  attachRefusal?: { status: number; code?: string };
  /** `pages.get_sharing` is a 404: the server does not offer mail and Scout in pages. */
  off?: boolean;
}

export interface SaveServer {
  routes: Record<string, Handler>;
  docs: Map<string, Y.Doc>;
  /** Files the pages hold because of a message. */
  held: Set<string>;
  /** Pages trashed. */
  trashed: Set<string>;
  /** Every call, "METHOD /path". */
  calls: string[];
}

/** The pages API for Save to page, over state the calls change. */
export function saveToPageServer(options: SaveServerOptions = {}): SaveServer {
  const docs = new Map<string, Y.Doc>();
  const held = new Set<string>();
  const trashed = new Set<string>();
  const calls: string[] = [];
  const pages = new Map<string, Schema<"PageSummary">>(
    WRITABLE.map((page) => [page.id, page] as const),
  );
  const docOf = (id: string): Y.Doc => {
    let doc = docs.get(id);
    if (doc === undefined) {
      doc = plainDoc(`Notes for ${pages.get(id)?.title ?? "a page"}.`);
      docs.set(id, doc);
    }
    return doc;
  };
  const linked = options.linked ?? [CLIENT_ONBOARDING.id];
  const note =
    (route: string, handler: Handler): Handler =>
    (body, url) => {
      calls.push(route);
      return handler(body, url);
    };
  const routes: Record<string, Handler> = {};
  const add = (route: string, handler: Handler) => {
    routes[route] = note(route, handler);
  };
  add(`GET ${W}/pages`, () => ({ body: { pages: [...pages.values()], tables: false } }));
  add(`GET ${W}/members`, () => ({ body: { members: MEMBERS } }));
  add(`GET ${W}/pages/backlinks`, (_body, url) => ({
    body: {
      // What links to a page is another page (Team handbook); what links to the
      // thread is `linked`.
      pages: (url.searchParams.get("type") === "pages.page" ? [HANDBOOK.id] : linked).flatMap(
        (id) => {
          const page = pages.get(id);
          return page === undefined
            ? []
            : [
                {
                  page: id,
                  title: page.title,
                  place: page.place,
                  icon: page.icon ?? null,
                  edited_at: page.edited_at,
                },
              ];
        },
      ),
    },
  }));
  add(`POST ${W}/pages`, (body) => {
    const { title, place } = body as { title?: string; place?: Schema<"Place"> };
    const made: Schema<"PageSummary"> = {
      ...ACME,
      id: "0192f1a0-0000-7000-8000-0000000009f0",
      title: title ?? "",
      place: place ?? "private",
      parent: null,
      has_children: false,
    };
    pages.set(made.id, made);
    return { status: 201, body: viewOfDoc(made, docOf(made.id)) };
  });
  const perPage = [...WRITABLE.map((page) => page.id), "0192f1a0-0000-7000-8000-0000000009f0"];
  for (const id of perPage) {
    add(`GET ${W}/pages/${id}`, () => {
      const page = pages.get(id);
      return page === undefined ? { status: 404 } : { body: viewOfDoc(page, docOf(id)) };
    });
    add(`POST ${W}/pages/${id}/sync`, (body) => {
      const { state_vector: vector, update } = body as { state_vector: string; update?: string };
      const doc = docOf(id);
      if (update !== undefined) {
        Y.applyUpdate(doc, fromBase64(update));
      }
      return {
        body: {
          revision: 2,
          state_vector: toBase64(Y.encodeStateVector(doc)),
          update: toBase64(Y.encodeStateAsUpdate(doc, fromBase64(vector))),
        },
      };
    });
    add(`GET ${W}/pages/${id}/sharing`, () => {
      if (options.off === true) {
        return { status: 404 };
      }
      return {
        body: SHARING[id] ?? {
          everyone: false,
          notion: false,
          place: "private",
          readers: [ANA.user_id],
        },
      };
    });
    add(`POST ${W}/pages/${id}/files/from-message`, (body) => {
      if (options.attachRefusal !== undefined) {
        const { status, code } = options.attachRefusal;
        return { status, body: { status, code, title: "Refused" } };
      }
      const { attachments } = body as { attachments: string[] };
      const files = attachments.map((attachment) => {
        const found = KICKOFF_THREAD.messages
          .flatMap((message) => message.attachments)
          .find((file) => file.id === attachment);
        const blob = `hash-${attachment.slice(-4)}`;
        held.add(blob);
        return {
          attachment,
          blob,
          content_type:
            found?.name.endsWith(".zip") === true ? "application/zip" : "application/pdf",
          name: found?.name ?? "file",
          size: found?.size ?? 0,
        };
      });
      return { body: { files } };
    });
    add(`POST ${W}/pages/${id}/files/detach`, (body) => {
      const { files } = body as { files: string[] };
      for (const blob of files) {
        held.delete(blob);
      }
      return { body: { removed: files, kept: [] } };
    });
    add(`POST ${W}/pages/${id}/trash`, () => {
      trashed.add(id);
      const page = pages.get(id);
      pages.delete(id);
      return { body: { ...page, trashed_at: "2026-10-28T09:00:00Z" } };
    });
  }
  return { routes, docs, held, trashed, calls };
}

/** The same routes for the tests' `mockApi`, whose handlers also get the request. */
export function forRequests(
  routes: Record<string, Handler>,
): Record<string, (request: Request, body: unknown) => Answer> {
  return Object.fromEntries(
    Object.entries(routes).map(([route, handler]) => [
      route,
      (request: Request, body: unknown) => handler(body, new URL(request.url)),
    ]),
  );
}

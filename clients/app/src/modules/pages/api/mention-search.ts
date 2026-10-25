import { api, unwrap } from "~/lib/api";

import type { MentionType, PageSummary } from "../types";

/** One thing `@` can insert. */
export interface MentionCandidate {
  /** What it is. */
  type: MentionType;
  /** Its id (a person's cleaned address). */
  id: string;
  /** Its name. */
  label: string;
  /** A line under it (an address, a sender). */
  detail?: string | undefined;
}

/** The candidates of one kind, in the order the menu shows the kinds. */
export interface MentionGroup {
  kind: "people" | "threads" | "tasks" | "events" | "pages";
  candidates: MentionCandidate[];
}

/** Candidates kept per kind. */
const PER_KIND = 5;

const fold = (text: string) =>
  text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();

const matches = (text: string | null | undefined, query: string) =>
  query === "" || fold(text ?? "").includes(fold(query));

/** Run `look`, and answer nothing when it fails (the module may be off, the search busy). */
async function quietly<TResult>(look: () => Promise<TResult[]>): Promise<TResult[]> {
  try {
    return await look();
  } catch {
    return [];
  }
}

const day = (offsetDays: number) =>
  new Date(Date.now() + offsetDays * 86_400_000).toISOString().slice(0, 10);

/**
 * Search by name across people, threads, tasks, events and pages: what a
 * member typed after `@`. A person is found by name among the workspace's
 * members and the viewer's own correspondents, never typed as an address.
 * Every answer is the viewer's own: a record they cannot read is not offered.
 */
export async function searchMentions(
  workspaceId: string,
  query: string,
  pages: readonly PageSummary[],
  signal: AbortSignal,
): Promise<MentionGroup[]> {
  const path = { workspace_id: workspaceId };
  const typed = query.trim();
  const [members, correspondents, threads, tasks, events] = await Promise.all([
    quietly<MentionCandidate>(async () => {
      const { members: list } = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/members", { params: { path }, signal }),
      );
      return list
        .filter((member) => matches(member.display_name, typed) || matches(member.email, typed))
        .map((member) => ({
          type: "workspaces.member" as const,
          id: member.user_id,
          label: member.display_name ?? member.email ?? member.user_id,
          detail: member.email ?? undefined,
        }));
    }),
    quietly<MentionCandidate>(async () => {
      if (typed.length < 2) {
        return [];
      }
      const { correspondents: list } = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/correspondents", {
          params: { path, query: { q: typed, limit: PER_KIND } },
          signal,
        }),
      );
      return list.map((person) => ({
        type: "person" as const,
        id: person.address.toLowerCase(),
        label: person.name ?? person.address,
        detail: person.address,
      }));
    }),
    quietly<MentionCandidate>(async () => {
      if (typed.length < 2) {
        return [];
      }
      const results = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/search", {
          params: { path, query: { q: typed, prefix: true, limit: PER_KIND } },
          signal,
        }),
      );
      return results.hits
        .filter((hit) => hit.record.type === "conversations.conversation")
        .map((hit) => ({
          type: "conversations.conversation" as const,
          id: hit.record.id,
          label: hit.title.map((segment) => segment.text).join(""),
          detail: hit.from?.name ?? hit.from?.address ?? undefined,
        }));
    }),
    quietly<MentionCandidate>(async () => {
      const { tasks: list } = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tasks", {
          params: { path, query: { state: "open" } },
          signal,
        }),
      );
      return list
        .filter((task) => matches(task.title, typed))
        .map((task) => ({
          type: "tasks.task" as const,
          id: task.id,
          label: task.title,
        }));
    }),
    quietly<MentionCandidate>(async () => {
      const { events: list } = unwrap(
        await api.GET("/api/v1/w/{workspace_id}/schedule/events", {
          params: {
            path,
            query: {
              from: day(-30),
              to: day(180),
              limit: PER_KIND,
              ...(typed === "" ? {} : { q: typed }),
            },
          },
          signal,
        }),
      );
      return list.map((event) => ({
        type: "schedule.event" as const,
        id: event.id,
        label: event.title,
      }));
    }),
  ]);
  const pageCandidates = pages
    .filter((page) => matches(page.title, typed))
    .map((page) => ({
      type: "pages.page" as const,
      id: page.id,
      label: page.title,
    }));
  const groups: MentionGroup[] = [
    { kind: "people", candidates: [...members, ...correspondents].slice(0, PER_KIND * 2) },
    { kind: "threads", candidates: threads.slice(0, PER_KIND) },
    { kind: "tasks", candidates: tasks.slice(0, PER_KIND) },
    { kind: "events", candidates: events.slice(0, PER_KIND) },
    { kind: "pages", candidates: pageCandidates.slice(0, PER_KIND) },
  ];
  return groups.filter((group) => group.candidates.length > 0);
}

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, unwrapEmpty, type Schema } from "~/lib/api";

export type PersonNote = Schema<"PersonNote">;

/** Longest note the server keeps, in characters. */
export const MAX_NOTE_CHARS = 500;

// Keyed by operationId: the list is the member's own, refetched after each
// change they make here.
const keys = {
  notes: (workspaceId: string) =>
    operationKey("conversations.list_person_notes", { workspace_id: workspaceId }),
};

/**
 * `conversations.list_person_notes`: what you told Scout about people, the
 * newest change first. Yours alone; nobody else in the workspace sees them.
 */
export function usePersonNotes(
  workspaceId: string,
  { enabled = true }: { enabled?: boolean } = {},
) {
  return useQuery({
    queryKey: keys.notes(workspaceId),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/person-notes", {
          params: { path: { workspace_id: workspaceId } },
          signal,
        }),
      ).notes,
  });
}

/**
 * Keep a note (`conversations.set_person_note`, by address, in the body:
 * never in a URL) and forget one (`conversations.delete_person_note`).
 */
export function usePersonNoteMutations(workspaceId: string) {
  const queryClient = useQueryClient();
  const settle = () => queryClient.invalidateQueries({ queryKey: keys.notes(workspaceId) });
  const keep = useMutation({
    mutationFn: async ({ address, note }: { address: string; note: string }) =>
      unwrap(
        await api.PUT("/api/v1/w/{workspace_id}/person-notes", {
          params: { path: { workspace_id: workspaceId } },
          body: { address, note },
        }),
      ),
    onSuccess: (kept) => {
      // Shown at once, before the list is asked again.
      queryClient.setQueryData<PersonNote[]>(keys.notes(workspaceId), (list) => [
        kept,
        ...(list ?? []).filter((item) => item.id !== kept.id),
      ]);
    },
    onSettled: settle,
  });
  const forget = useMutation({
    mutationFn: async ({ noteId }: { noteId: string }) => {
      unwrapEmpty(
        await api.DELETE("/api/v1/w/{workspace_id}/person-notes/{note_id}", {
          params: { path: { workspace_id: workspaceId, note_id: noteId } },
        }),
      );
    },
    onSuccess: (_, { noteId }) => {
      queryClient.setQueryData<PersonNote[]>(keys.notes(workspaceId), (list) =>
        list?.filter((item) => item.id !== noteId),
      );
    },
    onSettled: settle,
  });
  return { keep, forget };
}

/**
 * Whether `address` is plain enough for Scout to look up, as the server
 * checks it: one `@` with something on both sides, and no quote, backslash,
 * whitespace or control character.
 */
function plainAddress(address: string): boolean {
  const parts = address.split("@");
  if (parts.length !== 2 || parts[0] === "" || parts[1] === "") {
    return false;
  }
  return ![...address].some((c) => c === '"' || c === "\\" || /\s/u.test(c) || /\p{Cc}/u.test(c));
}

/** A party of a message: its name and address. */
interface Party {
  name?: string | null | undefined;
  address: string;
}

/**
 * The person a reply to `message` is written for, as the server reads them
 * for Scout: the sender of a message received, the first recipient of one
 * sent. `undefined` when there is none or their address is not plain.
 */
export function answeredPerson(message: {
  direction: string;
  from: Party;
  to: readonly Party[];
}): { address: string; name: string | undefined } | undefined {
  const party = message.direction === "outbound" ? message.to[0] : message.from;
  if (party === undefined) {
    return undefined;
  }
  const address = party.address.trim().toLowerCase();
  if (!plainAddress(address)) {
    return undefined;
  }
  const name = party.name?.trim();
  return { address, name: name === undefined || name === "" ? undefined : name };
}

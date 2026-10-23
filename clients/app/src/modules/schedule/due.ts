import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { useUndoToast } from "~/lib/ui";

/**
 * The Due layer (the follow-up of this change that needs the `tasks`
 * contracts): the member's open tasks that have a due date, drawn on their
 * day. Until `tasks` publishes them there is nothing to read, so these hooks
 * answer an empty list and the calendar shows no Due row. The shape below is
 * the one `design.md` documents (`tasks.task_changed.v1` plus the
 * `TaskSource` port for the title); a story seeds it through the query cache.
 */
export interface DueItem {
  id: string;
  /** Do something, or pay something. */
  verb: "do" | "pay";
  title: string;
  /** Only when the task has one; never guessed. */
  amount?: { minor: number; currency: string } | null;
  /** A date (`2026-10-30`), or a time with an offset when it is due at a time. */
  due: string;
  /** The message it came from, when it came from one. */
  source?: {
    conversationId: string;
    stream: string;
    subject: string;
    sender: string;
    sentAt: string | null;
    quote?: string | null;
  } | null;
  /** The member holds time for it on its due day: only the held block shows. */
  held?: boolean;
}

export type DueAction = "done" | "hold" | "later";

export const dueKey = (workspaceId: string) => ["schedule.due", workspaceId] as const;
export const DUE_ACTION_KEY = ["schedule.due_action"] as const;

/** The open tasks with a due date. Empty until the `tasks` contracts are wired. */
export function useDueItems(workspaceId: string): readonly DueItem[] {
  // Nothing to ask yet: the cache is the only source (a story, or the `tasks`
  // follow-up, fills it), and the query is switched on when there is a server.
  const query = useQuery<DueItem[]>({ queryKey: dueKey(workspaceId), queryFn: skipToken });
  return query.data ?? [];
}

/**
 * Paid / Done, Hold time, Later on a Due row. The mutation itself is the
 * `tasks` follow-up's (`setMutationDefaults(DUE_ACTION_KEY, …)`); each
 * success is offered with Undo like every other action.
 */
export function useDueActions(workspaceId: string) {
  const queryClient = useQueryClient();
  const undo = useUndoToast();
  return useMutation<
    { label: string; undo: () => void } | undefined,
    Error,
    { item: DueItem; action: DueAction }
  >({
    mutationKey: DUE_ACTION_KEY,
    onSuccess: (result) => {
      if (result !== undefined) {
        undo.push({ label: result.label, undo: result.undo });
      }
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: dueKey(workspaceId) }),
  });
}

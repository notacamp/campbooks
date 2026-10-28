import { useTasks, type TaskView } from "../api/queries";
import { CONVERSATION } from "../source";

import { useTaskActions } from "./use-task-actions";

/** A task of the member that asks "Is it done?" about a conversation, and how to answer. */
export interface ReplyQuestion {
  task: TaskView;
  /** Done: closes the task, with an Undo. */
  done: () => void;
  /** Not yet: the task stays open and is not asked about again until a newer reply. */
  notYet: () => void;
}

/**
 * The open task of the member that came from `conversationId` and asks
 * "You replied. Is it done?" (campbooks-cloud#396), for the conversation's
 * "where things stand" note; `undefined` when none asks.
 */
export function useReplyQuestion(
  workspaceId: string,
  conversationId: string,
): ReplyQuestion | undefined {
  const open = useTasks(workspaceId);
  const actions = useTaskActions(workspaceId);
  const task = open.data?.find(
    (candidate) =>
      candidate.asks_done &&
      candidate.source?.type === CONVERSATION &&
      candidate.source.id === conversationId,
  );
  if (task === undefined) {
    return undefined;
  }
  return {
    task,
    done: () => {
      actions.done(task);
    },
    notYet: () => {
      actions.notYet(task);
    },
  };
}

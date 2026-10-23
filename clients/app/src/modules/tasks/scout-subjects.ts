import { useTasksByIds } from "./api/queries";

/** What a chat with Scout is about, in one line, or why it can't be said. */
interface ChatSubject {
  /** The line, once read. */
  text: string | undefined;
  /** It could not be read: gone, or no longer yours to see. */
  failed: boolean;
}

/**
 * A task as a chat about it names it: its title (campbooks-cloud#43). Asks
 * for nothing without a task.
 */
export function useTaskSubject(workspaceId: string, taskId: string | undefined): ChatSubject {
  const found = useTasksByIds(workspaceId, taskId === undefined ? [] : [taskId]);
  const task = found.data?.find((candidate) => candidate.id === taskId);
  return {
    text: task?.title,
    failed: found.isError || (found.isSuccess && task === undefined),
  };
}

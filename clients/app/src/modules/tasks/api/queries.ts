import { useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";

import { api, operationKey, unwrap, type Schema } from "~/lib/api";

export type TaskView = Schema<"TaskView">;
export type TaskList = Schema<"TaskList">;
export type TaskVerb = Schema<"TaskVerb">;
export type NewTask = Schema<"NewTask">;

// Keyed by operationId, so the change feed refetches them when a
// `tasks.task` changes.
const LIST = "tasks.list";

const keys = {
  all: (workspaceId: string) => operationKey(LIST, { workspace_id: workspaceId }),
  open: (workspaceId: string) =>
    operationKey(LIST, { workspace_id: workspaceId, state: "open", whose: "mine" }),
  handedOff: (workspaceId: string) =>
    operationKey(LIST, { workspace_id: workspaceId, state: "open", whose: "handed_off" }),
  ids: (workspaceId: string, ids: readonly string[]) =>
    operationKey(LIST, { workspace_id: workspaceId, ids: [...ids].toSorted().join(",") }),
};

/** The member's offset from UTC in minutes: where their day starts. */
const utcOffset = () => -new Date().getTimezoneOffset();

/** `tasks.list`: the caller's open tasks, overdue first, then by due date, then undated. */
export function useTasks(workspaceId: string, me?: string) {
  return useQuery({
    queryKey: keys.open(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tasks", {
          params: {
            path: { workspace_id: workspaceId },
            query: { state: "open", whose: "mine", utc_offset: utcOffset() },
          },
          signal,
        }),
      ),
    // A task closed or handed to someone while the list shows it leaves it at once.
    select: (list: TaskList) =>
      list.tasks.filter(
        (task) => task.state === "open" && (me === undefined || task.assignee === me),
      ),
  });
}

/** `tasks.list` with `whose=handed_off`: the open tasks the caller gave to someone. */
export function useHandedOff(workspaceId: string, me?: string) {
  return useQuery({
    queryKey: keys.handedOff(workspaceId),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tasks", {
          params: {
            path: { workspace_id: workspaceId },
            query: { state: "open", whose: "handed_off", utc_offset: utcOffset() },
          },
          signal,
        }),
      ),
    select: (list: TaskList) =>
      list.tasks.filter(
        (task) => task.state === "open" && (me === undefined || task.handed_by === me),
      ),
  });
}

/**
 * `tasks.list?ids=`: Home's batch read of the tasks its items point at (1 to
 * 100; those the caller cannot see are left out).
 */
export function useTasksByIds(workspaceId: string, ids: readonly string[]) {
  const wanted = ids.slice(0, 100);
  return useQuery({
    queryKey: keys.ids(workspaceId, wanted),
    enabled: wanted.length > 0,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/api/v1/w/{workspace_id}/tasks", {
          params: {
            path: { workspace_id: workspaceId },
            query: { ids: wanted.join(","), utc_offset: utcOffset() },
          },
          signal,
        }),
      ),
    select: (list: TaskList) => list.tasks,
  });
}

/** How many tasks are open for the caller (the nav's count); `undefined` until known. */
export function useTaskCount(workspaceId: string): number | undefined {
  return useTasks(workspaceId).data?.length;
}

/** Whether a cached list is one of the page's own open lists (mine or handed off). */
function isPageList(key: readonly unknown[]): boolean {
  const params = key[1];
  return (
    typeof params === "object" &&
    params !== null &&
    "whose" in params &&
    (params.whose === "mine" || params.whose === "handed_off")
  );
}

/**
 * Replace task `id` in every cached list of the workspace with `next(task)`.
 * A task that is open afterwards and is missing from one of the page's open
 * lists (a task taken back is mine again, one handed off is the handed-off
 * list's) is put there at once; each list's own filter decides whose it is,
 * and the refetch that follows has the last word.
 */
export function patchTask(
  queryClient: QueryClient,
  workspaceId: string,
  id: string,
  next: (task: TaskView) => TaskView,
): void {
  const lists = queryClient.getQueryCache().findAll({ queryKey: keys.all(workspaceId) });
  const known = lists
    .flatMap((query) => (query.state.data as TaskList | undefined)?.tasks ?? [])
    .find((task) => task.id === id);
  const after = known === undefined ? undefined : next(known);
  for (const query of lists) {
    const list = query.state.data as TaskList | undefined;
    if (list === undefined) {
      continue;
    }
    if (list.tasks.some((task) => task.id === id)) {
      queryClient.setQueryData<TaskList>(query.queryKey, {
        ...list,
        tasks: list.tasks.map((task) => (task.id === id ? next(task) : task)),
      });
    } else if (after?.state === "open" && isPageList(query.queryKey)) {
      queryClient.setQueryData<TaskList>(query.queryKey, {
        ...list,
        tasks: [...list.tasks, after],
      });
    }
  }
}

/** Refetch every task list of the workspace, and Home (which lists the same tasks as items). */
export function useRefreshTasks(workspaceId: string) {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: keys.all(workspaceId) });
    void queryClient.invalidateQueries({
      queryKey: operationKey("attention.list", { workspace_id: workspaceId }),
    });
  };
}

/** Task `id`'s path parameters. */
const at = (workspaceId: string, id: string) => ({
  params: {
    path: { workspace_id: workspaceId, task_id: id },
    query: { utc_offset: utcOffset() },
  },
});

/** What a plan change sends. */
export type PlanChange =
  { plan: "none" } | { plan: "date"; plan_on: string } | { plan: "later"; later_until: string };

/** What `tasks.hold` sends: when, and on which calendar (the Campbooks one when left out). */
export interface HoldChange {
  starts: string;
  ends: string;
  calendarId?: string | undefined;
}

/** What `tasks.update` makes of a task: the whole editable set. */
interface TaskEdit {
  title: string;
  verb: TaskVerb;
  due_on: string | null;
  due_at: string | null;
}

/** The task's editable set as it is now. */
export const editOf = (task: TaskView): TaskEdit => ({
  title: task.title,
  verb: task.verb,
  due_on: task.due_on ?? null,
  due_at: task.due_at ?? null,
});

/** The calls a task takes, each with the revision seen, each answering the task afterwards. */
export function useTaskCalls(workspaceId: string) {
  const queryClient = useQueryClient();
  const keep = (view: TaskView) => {
    patchTask(queryClient, workspaceId, view.id, () => view);
    return view;
  };
  return {
    create: async (body: NewTask): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks", {
            params: { path: { workspace_id: workspaceId }, query: { utc_offset: utcOffset() } },
            body,
          }),
        ),
      ),
    update: async (task: TaskView, edit: TaskEdit): Promise<TaskView> =>
      keep(
        unwrap(
          await api.PATCH("/api/v1/w/{workspace_id}/tasks/{task_id}", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision, ...edit },
          }),
        ),
      ),
    plan: async (task: TaskView, change: PlanChange): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/plan", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision, ...change },
          }),
        ),
      ),
    done: async (task: TaskView): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/done", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision },
          }),
        ),
      ),
    notNeeded: async (task: TaskView): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/not_needed", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision },
          }),
        ),
      ),
    reopen: async (task: TaskView): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/reopen", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision },
          }),
        ),
      ),
    handOff: async (task: TaskView, to: string): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/hand_off", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision, to },
          }),
        ),
      ),
    hold: async (task: TaskView, change: HoldChange): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/hold", {
            ...at(workspaceId, task.id),
            body: {
              revision: task.revision,
              starts: change.starts,
              ends: change.ends,
              ...(change.calendarId === undefined ? {} : { calendar_id: change.calendarId }),
            },
          }),
        ),
      ),
    releaseHold: async (task: TaskView): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/release", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision },
          }),
        ),
      ),
    takeBack: async (task: TaskView): Promise<TaskView> =>
      keep(
        unwrap(
          await api.POST("/api/v1/w/{workspace_id}/tasks/{task_id}/take_back", {
            ...at(workspaceId, task.id),
            body: { revision: task.revision },
          }),
        ),
      ),
  };
}

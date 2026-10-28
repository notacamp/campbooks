import { createRoute, useParams, useSearch, type AnyRoute } from "@tanstack/react-router";

import { TasksPage } from "./components/TasksPage/TasksPage";
import type { HoldSources } from "./hold";
import type { UseTaskQuote, UseTaskSources } from "./source";

export interface TasksRouteOptions {
  /** Where tasks made from conversations came from (the conversations module's). */
  useSources: UseTaskSources;
  /** The sender's sentence that asked for a task (the conversations module's, access-checked). */
  useQuote?: UseTaskQuote | undefined;
  /** The member's free time and writable calendars, for Hold time (the schedule module's). */
  holdSources?: HoldSources | undefined;
}

/**
 * `/w/$workspaceId/tasks` and `/tasks/$taskId`: the Tasks place, with a task
 * open beside the list. `/tasks?new=1` opens the new-task form there instead.
 */
export function buildTasksRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useSources, useQuote, holdSources }: TasksRouteOptions,
) {
  function TasksRoute() {
    const params: { workspaceId?: string; taskId?: string } = useParams({ strict: false });
    const search: { new?: 1 } = useSearch({ strict: false });
    return (
      <TasksPage
        key={params.workspaceId}
        workspaceId={params.workspaceId ?? ""}
        taskId={params.taskId}
        creating={params.taskId === undefined && search.new === 1}
        useSources={useSources}
        useQuote={useQuote}
        holdSources={holdSources}
      />
    );
  }
  // `new` is the form's only state: present (1) or not.
  const validateSearch = (search: Record<string, unknown>): { new?: 1 } =>
    search["new"] === 1 || search["new"] === "1" || search["new"] === true ? { new: 1 } : {};
  const list = createRoute({
    getParentRoute: () => parent,
    path: "/tasks",
    validateSearch,
    component: TasksRoute,
  });
  const task = createRoute({
    getParentRoute: () => parent,
    path: "/tasks/$taskId",
    component: TasksRoute,
  });
  return [list, task] as const;
}

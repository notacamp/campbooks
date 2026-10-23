import { createRoute, useParams, useSearch, type AnyRoute } from "@tanstack/react-router";

import { TasksPage } from "./components/TasksPage/TasksPage";
import type { UseTaskSources } from "./source";

export interface TasksRouteOptions {
  /** Where tasks made from conversations came from (the conversations module's). */
  useSources: UseTaskSources;
}

/**
 * `/w/$workspaceId/tasks` and `/tasks/$taskId`: the Tasks place, with a task
 * open beside the list. `?add=` changes whenever `n t` asks to add a task
 * from another page.
 */
export function buildTasksRoutes<TParent extends AnyRoute>(
  parent: TParent,
  { useSources }: TasksRouteOptions,
) {
  function TasksRoute() {
    const params: { workspaceId?: string; taskId?: string } = useParams({ strict: false });
    const search: { add?: number } = useSearch({ strict: false });
    return (
      <TasksPage
        key={params.workspaceId}
        workspaceId={params.workspaceId ?? ""}
        taskId={params.taskId}
        addRequest={search.add}
        useSources={useSources}
      />
    );
  }
  const validateSearch = (search: Record<string, unknown>): { add?: number } => {
    const add = Number(search["add"]);
    return Number.isFinite(add) && add > 0 ? { add } : {};
  };
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

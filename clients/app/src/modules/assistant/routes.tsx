import { createRoute, useNavigate, useParams, type AnyRoute } from "@tanstack/react-router";
import type { ComponentType } from "react";

import { MemoryPage } from "./components/MemoryPage/MemoryPage";
import { Questions } from "./components/Questions/Questions";

/** The rules list Scout's memory shows (the conversations module's, plugged in by the app). */
type MemoryRulesSlot = ComponentType<{ workspaceId: string }>;
/** What the member told Scout about people (the conversations module's, plugged in by the app). */
type MemoryPeopleSlot = ComponentType<{ workspaceId: string }>;

export interface AssistantRouteParents<TRoot extends AnyRoute, TWorkspace extends AnyRoute> {
  /** Pages outside any workspace's frame, behind the session gate. */
  root: TRoot;
  /** `/w/$workspaceId`: pages inside a workspace's frame. */
  workspace: TWorkspace;
  /** What Scout's memory lists under "Your rules". */
  rules?: MemoryRulesSlot | undefined;
  /** What Scout's memory lists under "People you told Scout about". */
  people?: MemoryPeopleSlot | undefined;
}

/**
 * The questions (`/scout/$workspaceId`, a one-question-per-view page outside
 * the workspace's frame) and Scout's memory (`/w/$workspaceId/settings/memory`).
 */
export function buildAssistantRoutes<TRoot extends AnyRoute, TWorkspace extends AnyRoute>({
  root,
  workspace,
  rules,
  people,
}: AssistantRouteParents<TRoot, TWorkspace>) {
  const questions = createRoute({
    getParentRoute: () => root,
    path: "/scout/$workspaceId",
    component: function QuestionsRoute() {
      const { workspaceId } = useParams({ strict: false });
      const navigate = useNavigate();
      const id = workspaceId ?? "";
      return (
        <Questions
          workspaceId={id}
          onHome={() => {
            void navigate({ to: "/w/$workspaceId", params: { workspaceId: id } });
          }}
          onMemory={() => {
            void navigate({ to: `/w/${id}/settings/memory` });
          }}
        />
      );
    },
  });
  const memory = createRoute({
    getParentRoute: () => workspace,
    path: "/settings/memory",
    component: function MemoryRoute() {
      const { workspaceId } = useParams({ strict: false });
      const navigate = useNavigate();
      const id = workspaceId ?? "";
      return (
        <MemoryPage
          workspaceId={id}
          rules={rules}
          people={people}
          onQuestions={() => {
            void navigate({ to: `/scout/${id}` });
          }}
        />
      );
    },
  });
  return { root: [questions] as const, workspace: [memory] as const };
}

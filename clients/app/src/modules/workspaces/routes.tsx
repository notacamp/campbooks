import {
  createRoute,
  useNavigate,
  useParams,
  useSearch,
  type AnyRoute,
} from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { signOutAndLeave } from "~/lib/api";
import { FocusPage, Visor } from "~/lib/ui";

import { useWorkspaces } from "./api/queries";
import { AcceptInvitation } from "./components/AcceptInvitation/AcceptInvitation";
import { CreateWorkspace } from "./components/CreateWorkspace/CreateWorkspace";
import { MembersPage } from "./components/MembersPage/MembersPage";

const leave = () => {
  void signOutAndLeave();
};

function Waiting() {
  const { t } = useTranslation("shell");
  return <FocusPage leading={<Visor state="thinking" size="xl" announce />} title={t("loading")} />;
}

function CreateWorkspaceRoute() {
  const navigate = useNavigate();
  const workspaces = useWorkspaces();
  if (workspaces.isPending) {
    return <Waiting />;
  }
  const firstRun = (workspaces.data?.length ?? 0) === 0;
  return (
    <CreateWorkspace
      firstRun={firstRun}
      onSignOut={leave}
      onCancel={
        firstRun
          ? undefined
          : () => {
              void navigate({ to: "/" });
            }
      }
      onOpen={(workspace) => {
        void navigate({ to: "/w/$workspaceId", params: { workspaceId: workspace.id } });
      }}
      onInvite={(workspace) => {
        void navigate({
          to: "/w/$workspaceId/settings/members",
          params: { workspaceId: workspace.id },
        });
      }}
    />
  );
}

function AcceptInvitationRoute({ token }: { token: string | undefined }) {
  const navigate = useNavigate();
  return (
    <AcceptInvitation
      token={token}
      onSignOut={leave}
      onHome={() => {
        void navigate({ to: "/" });
      }}
      onOpen={(workspace) => {
        void navigate({ to: "/w/$workspaceId", params: { workspaceId: workspace.id } });
      }}
    />
  );
}

function MembersRoute() {
  const { workspaceId } = useParams({ strict: false });
  const workspaces = useWorkspaces();
  const workspace = workspaces.data?.find((candidate) => candidate.id === workspaceId);
  // The workspace layout only renders its pages for a workspace in the list.
  return workspace === undefined ? null : <MembersPage workspace={workspace} />;
}

export interface WorkspacesRouteParents<TRoot extends AnyRoute, TWorkspace extends AnyRoute> {
  /** The app's root route (behind the session gate). */
  root: TRoot;
  /** The `/w/$workspaceId` layout. */
  workspace: TWorkspace;
}

export function buildWorkspacesRoutes<TRoot extends AnyRoute, TWorkspace extends AnyRoute>({
  root,
  workspace,
}: WorkspacesRouteParents<TRoot, TWorkspace>) {
  const create = createRoute({
    getParentRoute: () => root,
    path: "/welcome",
    component: CreateWorkspaceRoute,
  });
  const accept = createRoute({
    getParentRoute: () => root,
    path: "/invitations/accept",
    validateSearch: (search: Record<string, unknown>): { token?: string } =>
      typeof search["token"] === "string" ? { token: search["token"] } : {},
    component: function AcceptRoute() {
      const { token } = useSearch({ strict: false });
      return <AcceptInvitationRoute token={token} />;
    },
  });
  const members = createRoute({
    getParentRoute: () => workspace,
    path: "/settings/members",
    component: MembersRoute,
  });
  return { root: [create, accept] as const, workspace: [members] as const };
}

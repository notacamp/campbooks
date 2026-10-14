import { useQuery } from "@tanstack/react-query";
import { useId, type ComponentType } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Avatar,
  Button,
  ButtonLink,
  EmptyState,
  Fact,
  FactList,
  Heading,
  List,
  ListRow,
  SkeletonList,
} from "~/lib/ui";

import { adminWorkspaceQuery, type AdminWorkspaceDetail } from "../../api/admin";
import { AdminDetailFrame, useDateFormat, useRouterLink } from "../AdminList/AdminList";

/** What another module shows about a workspace's connections (the connectors module's view). */
export type ConnectionsPanel = ComponentType<{ workspaceId: string }>;

export interface AdminWorkspacePageProps {
  workspaceId: string;
  /** Its connected accounts and how they sync. */
  ConnectionsPanel: ConnectionsPanel;
}

/** Admin → Workspaces → one workspace: what it is, who is in it, how its mailboxes are doing. */
export function AdminWorkspacePage({ workspaceId, ConnectionsPanel }: AdminWorkspacePageProps) {
  const { t } = useTranslation("workspaces");
  const workspace = useQuery(adminWorkspaceQuery(workspaceId));
  const frame = {
    backLabel: t("admin.workspaces.back"),
    backHref: "/admin/workspaces",
  };

  if (workspace.isPending) {
    return (
      <AdminDetailFrame title={t("admin.workspace.loading")} {...frame}>
        <div role="status" aria-label={t("admin.workspace.loading")}>
          <SkeletonList rows={4} />
        </div>
      </AdminDetailFrame>
    );
  }
  if (workspace.isError) {
    const missing = errorStatus(workspace.error) === 404;
    return (
      <AdminDetailFrame title={t("admin.workspaces.title")} {...frame}>
        <EmptyState
          title={missing ? t("admin.workspace.notFound") : t("admin.workspace.loadError")}
          visor={missing ? "unsure" : "thinking"}
          visorSize="lg"
          action={
            missing ? undefined : (
              <Button
                onClick={() => {
                  void workspace.refetch();
                }}
              >
                {t("admin.retry")}
              </Button>
            )
          }
        >
          {missing ? t("admin.workspace.notFoundLead") : t("admin.loadErrorLead")}
        </EmptyState>
      </AdminDetailFrame>
    );
  }
  return (
    <AdminDetailFrame title={workspace.data.workspace.name} {...frame}>
      <Workspace detail={workspace.data} />
      <Members detail={workspace.data} />
      <ConnectionsPanel workspaceId={workspaceId} />
    </AdminDetailFrame>
  );
}

function Workspace({ detail }: { detail: AdminWorkspaceDetail }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const { workspace } = detail;
  const deletedAt = workspace.deleted_at ?? undefined;
  return (
    <FactList>
      <Fact
        label={t("admin.workspace.state")}
        status={deletedAt === undefined ? "ok" : "off"}
        detail={deletedAt === undefined ? undefined : t("admin.workspace.purgedLater")}
      >
        {deletedAt === undefined
          ? t("admin.workspace.active")
          : t("admin.deletedOn", { date: formatDate(deletedAt) })}
      </Fact>
      <Fact label={t("admin.workspace.kind")}>{t(`kind.${workspace.kind}`)}</Fact>
      <Fact label={t("admin.workspace.created")}>{formatDate(workspace.created_at)}</Fact>
      <Fact label={t("admin.workspace.members")}>
        {t("admin.memberCount", { count: workspace.member_count })}
      </Fact>
      <Fact label={t("admin.workspace.pendingInvitations")}>
        {t("admin.workspace.invitationCount", { count: detail.pending_invitations })}
      </Fact>
    </FactList>
  );
}

function Members({ detail }: { detail: AdminWorkspaceDetail }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const link = useRouterLink();
  const titleId = useId();
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("admin.workspace.members")}
      </Heading>
      <List label={t("members.list", { workspace: detail.workspace.name })}>
        {detail.members.map((member) => {
          const name = member.display_name ?? member.email ?? member.user_id;
          const meta = [
            member.email === name ? null : member.email,
            t("members.joined", { date: formatDate(member.joined_at) }),
          ]
            .filter((part) => part !== null && part !== undefined)
            .join(" · ");
          return (
            <ListRow
              key={member.user_id}
              leading={<Avatar name={name} />}
              title={name}
              badge={t(`role.${member.role}`)}
              meta={meta}
              trailing={
                <ButtonLink
                  variant="ghost"
                  icon="chevron-right"
                  aria-label={t("admin.openLabel", { name })}
                  {...link(`/admin/people/${member.user_id}`)}
                >
                  {t("admin.open")}
                </ButtonLink>
              }
            />
          );
        })}
      </List>
    </section>
  );
}

import { useQuery } from "@tanstack/react-query";
import type { ComponentType } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Button,
  ButtonLink,
  EmptyState,
  Fact,
  FactList,
  SettingsSection,
  List,
  ListRow,
  SkeletonList,
  Text,
} from "~/lib/ui";

import { adminUserQuery, type AdminUserDetail } from "../../api/admin";
import { AdminDetailFrame, useDateFormat, useRouterLink } from "../AdminList/AdminList";

import { personName } from "./AdminPeoplePanel";

/** What another module shows about a person's account (the identity module's view). */
export type AccountPanel = ComponentType<{ accountId: string; name: string }>;

export interface AdminPersonPageProps {
  userId: string;
  /** Their account: how they sign in, and suspending it. */
  AccountPanel: AccountPanel;
}

/** Admin → People → one person: who they are, their account and their workspaces. */
export function AdminPersonPage({ userId, AccountPanel }: AdminPersonPageProps) {
  const { t } = useTranslation("workspaces");
  const person = useQuery(adminUserQuery(userId));
  const frame = {
    backLabel: t("admin.people.back"),
    backHref: "/admin/people",
  };

  if (person.isPending) {
    return (
      <AdminDetailFrame title={t("admin.person.loading")} {...frame}>
        <div role="status" aria-label={t("admin.person.loading")}>
          <SkeletonList rows={4} />
        </div>
      </AdminDetailFrame>
    );
  }
  if (person.isError) {
    const missing = errorStatus(person.error) === 404;
    return (
      <AdminDetailFrame title={t("admin.people.title")} {...frame}>
        <EmptyState
          title={missing ? t("admin.person.notFound") : t("admin.person.loadError")}
          visor={missing ? "unsure" : "thinking"}
          visorSize="lg"
          action={
            missing ? undefined : (
              <Button
                onClick={() => {
                  void person.refetch();
                }}
              >
                {t("admin.retry")}
              </Button>
            )
          }
        >
          {missing ? t("admin.person.notFoundLead") : t("admin.loadErrorLead")}
        </EmptyState>
      </AdminDetailFrame>
    );
  }
  const name = personName(person.data.user);
  return (
    <AdminDetailFrame title={name} {...frame}>
      <Person detail={person.data} />
      <AccountPanel accountId={person.data.user.account_id} name={name} />
      <PersonWorkspaces detail={person.data} />
    </AdminDetailFrame>
  );
}

function Person({ detail }: { detail: AdminUserDetail }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const { user } = detail;
  return (
    <FactList>
      <Fact label={t("admin.person.email")}>{user.email ?? t("admin.person.noEmail")}</Fact>
      <Fact label={t("admin.person.firstSignIn")}>{formatDate(user.created_at)}</Fact>
      <Fact label={t("admin.person.workspaces")}>
        {t("admin.workspaceCount", { count: user.workspace_count })}
      </Fact>
    </FactList>
  );
}

function PersonWorkspaces({ detail }: { detail: AdminUserDetail }) {
  const { t } = useTranslation("workspaces");
  const formatDate = useDateFormat();
  const link = useRouterLink();
  return (
    <SettingsSection title={t("admin.person.workspaces")} count={detail.workspaces.length}>
      {detail.workspaces.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("admin.person.noWorkspaces")}
        </Text>
      ) : (
        <List label={t("admin.person.workspacesOf", { name: personName(detail.user) })}>
          {detail.workspaces.map((workspace) => (
            <ListRow
              key={workspace.id}
              muted={workspace.deleted_at !== null && workspace.deleted_at !== undefined}
              title={workspace.name}
              badge={t(`role.${workspace.role}`)}
              meta={
                workspace.deleted_at === null || workspace.deleted_at === undefined
                  ? t("admin.person.joined", { date: formatDate(workspace.joined_at) })
                  : t("admin.deletedOn", { date: formatDate(workspace.deleted_at) })
              }
              trailing={
                <ButtonLink
                  variant="ghost"
                  icon="chevron-right"
                  aria-label={t("admin.openLabel", { name: workspace.name })}
                  {...link(`/admin/workspaces/${workspace.id}`)}
                >
                  {t("admin.open")}
                </ButtonLink>
              }
            />
          ))}
        </List>
      )}
    </SettingsSection>
  );
}

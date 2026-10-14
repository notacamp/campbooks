import { useInfiniteQuery } from "@tanstack/react-query";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { ButtonLink, List, ListRow } from "~/lib/ui";

import { adminWorkspacesQuery } from "../../api/admin";
import { AdminListPanel, useDateFormat, useDebounced, useRouterLink } from "../AdminList/AdminList";

/**
 * Admin → Workspaces: every workspace of the instance, newest first,
 * searchable by name. Metadata only: kind, age, members.
 */
export function AdminWorkspacesPanel() {
  const { t } = useTranslation("workspaces");
  const [search, setSearch] = useState("");
  const settled = useDebounced(search);
  const pages = useInfiniteQuery(adminWorkspacesQuery(settled));
  const link = useRouterLink();
  const formatDate = useDateFormat();
  const workspaces = pages.data?.pages.flatMap((page) => page.workspaces) ?? [];

  return (
    <AdminListPanel
      title={t("admin.workspaces.title")}
      lead={t("admin.workspaces.lead")}
      searchLabel={t("admin.workspaces.search")}
      searchPlaceholder={t("admin.workspaces.searchPlaceholder")}
      search={search}
      onSearch={setSearch}
      pages={pages}
      loadingLabel={t("admin.workspaces.loading")}
      emptyTitle={(q) =>
        q === "" ? t("admin.workspaces.empty") : t("admin.workspaces.noMatch", { search: q })
      }
      errorTitle={t("admin.workspaces.loadError")}
      errorLead={t("admin.loadErrorLead")}
      rows={
        workspaces.length === 0 ? undefined : (
          <List label={t("admin.workspaces.list")}>
            {workspaces.map((workspace) => {
              const deleted = workspace.deleted_at !== null && workspace.deleted_at !== undefined;
              const meta = [
                t(`kind.${workspace.kind}`),
                t("admin.memberCount", { count: workspace.member_count }),
                deleted
                  ? t("admin.deletedOn", { date: formatDate(workspace.deleted_at ?? "") })
                  : t("admin.workspaces.created", { date: formatDate(workspace.created_at) }),
              ].join(" · ");
              return (
                <ListRow
                  key={workspace.id}
                  muted={deleted}
                  title={workspace.name}
                  meta={meta}
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
              );
            })}
          </List>
        )
      }
    />
  );
}

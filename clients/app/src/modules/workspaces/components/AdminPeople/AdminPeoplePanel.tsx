import { useInfiniteQuery } from "@tanstack/react-query";
import { useState, type ComponentType, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Avatar, ButtonLink, List, ListRow, Tag } from "~/lib/ui";

import { adminUsersQuery, type AdminUser } from "../../api/admin";
import { AdminListPanel, useDateFormat, useDebounced, useRouterLink } from "../AdminList/AdminList";

/** How an administrator names a person: their name, else their address, else their account. */
export function personName(user: Pick<AdminUser, "display_name" | "email" | "account_id">) {
  return user.display_name ?? user.email ?? user.account_id;
}

/**
 * What another module knows about the accounts on the list: which are
 * suspended (the identity module's view).
 */
export type AccountStandings = ComponentType<{
  accountIds: readonly string[];
  children: (suspended: ReadonlySet<string>) => ReactNode;
}>;

export interface AdminPeoplePanelProps {
  /** Which of the people listed are suspended, to mark them. */
  AccountStandings: AccountStandings;
}

/**
 * Admin → People: everyone who has signed in, newest first, searchable by
 * name or address. Metadata only: who, since when, in how many workspaces,
 * and whether their account is suspended.
 */
export function AdminPeoplePanel({ AccountStandings }: AdminPeoplePanelProps) {
  const { t } = useTranslation("workspaces");
  const [search, setSearch] = useState("");
  const settled = useDebounced(search);
  const pages = useInfiniteQuery(adminUsersQuery(settled));
  const link = useRouterLink();
  const formatDate = useDateFormat();
  const users = pages.data?.pages.flatMap((page) => page.users) ?? [];

  return (
    <AdminListPanel
      title={t("admin.people.title")}
      lead={t("admin.people.lead")}
      searchLabel={t("admin.people.search")}
      searchPlaceholder={t("admin.people.searchPlaceholder")}
      search={search}
      onSearch={setSearch}
      pages={pages}
      loadingLabel={t("admin.people.loading")}
      emptyTitle={(q) =>
        q === "" ? t("admin.people.empty") : t("admin.people.noMatch", { search: q })
      }
      errorTitle={t("admin.people.loadError")}
      errorLead={t("admin.loadErrorLead")}
      rows={
        users.length === 0 ? undefined : (
          <AccountStandings accountIds={users.map((user) => user.account_id)}>
            {(suspended) => (
              <List label={t("admin.people.list")}>
                {users.map((user) => {
                  const name = personName(user);
                  const meta = [
                    user.email === name ? null : user.email,
                    t("admin.people.joined", { date: formatDate(user.created_at) }),
                    t("admin.workspaceCount", { count: user.workspace_count }),
                  ]
                    .filter((part) => part !== null && part !== undefined)
                    .join(" · ");
                  return (
                    <ListRow
                      key={user.id}
                      leading={<Avatar name={name} />}
                      title={name}
                      badge={
                        suspended.has(user.account_id) ? (
                          <Tag tone="danger">{t("admin.people.suspended")}</Tag>
                        ) : undefined
                      }
                      meta={meta}
                      trailing={
                        <ButtonLink
                          variant="ghost"
                          icon="chevron-right"
                          aria-label={t("admin.openLabel", { name })}
                          {...link(`/admin/people/${user.id}`)}
                        >
                          {t("admin.open")}
                        </ButtonLink>
                      }
                    />
                  );
                })}
              </List>
            )}
          </AccountStandings>
        )
      }
    />
  );
}

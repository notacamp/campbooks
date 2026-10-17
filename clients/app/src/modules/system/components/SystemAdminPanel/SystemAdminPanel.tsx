import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { Button, EmptyState, SettingsPage, SkeletonList } from "~/lib/ui";

import { adminOverviewQuery } from "../../api/admin";

import { SystemOverview } from "./SystemOverview";

/**
 * Admin → System: the instance's version, health and configuration. The
 * admin layout already turned non-administrators away and asks for a fresh
 * sign-in when the API wants one; this page shows the answer.
 */
export function SystemAdminPanel() {
  const { t } = useTranslation("system");
  const overview = useQuery(adminOverviewQuery());

  return (
    <SettingsPage
      label={t("admin.title")}
      title={t("admin.title")}
      lead={t("admin.lead")}
      actions={
        <Button
          variant="ghost"
          disabled={overview.isFetching}
          onClick={() => {
            void overview.refetch();
          }}
        >
          {t("admin.refresh")}
        </Button>
      }
    >
      {overview.isPending ? (
        <div role="status" aria-label={t("admin.loading")}>
          <SkeletonList rows={4} />
        </div>
      ) : overview.isError ? (
        <EmptyState
          title={t("admin.loadError.title")}
          visor="thinking"
          visorSize="lg"
          action={
            <Button
              onClick={() => {
                void overview.refetch();
              }}
            >
              {t("admin.loadError.retry")}
            </Button>
          }
        >
          {t("admin.loadError.lead")}
        </EmptyState>
      ) : (
        <SystemOverview overview={overview.data} />
      )}
    </SettingsPage>
  );
}

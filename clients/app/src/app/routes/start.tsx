import { Navigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { readPref } from "~/lib/storage/prefs";
import { Button, FocusPage, Visor } from "~/lib/ui";
import { useWorkspaces } from "~/modules/workspaces";

/**
 * `/`: opens the workspace last opened on this device if the user still
 * belongs to it, else their first one, else the first-run flow.
 */
export function Start() {
  const { t } = useTranslation();
  const workspaces = useWorkspaces();
  if (workspaces.isPending) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" announce />}
        title={t("gate.opening")}
      />
    );
  }
  if (workspaces.isError) {
    return (
      <FocusPage
        leading={<Visor state="thinking" size="xl" />}
        title={t("workspace.loadErrorTitle")}
        lead={t("workspace.loadErrorLead")}
        actions={
          <Button
            variant="primary"
            size="lg"
            onClick={() => {
              void workspaces.refetch();
            }}
          >
            {t("gate.retry")}
          </Button>
        }
      />
    );
  }
  const last = readPref("workspace");
  const target = workspaces.data.find((workspace) => workspace.id === last) ?? workspaces.data[0];
  return target === undefined ? (
    <Navigate to="/welcome" replace />
  ) : (
    <Navigate to="/w/$workspaceId" params={{ workspaceId: target.id }} replace />
  );
}

import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { Banner, Button, Fact, FactList, SettingsSection, SkeletonList, Text } from "~/lib/ui";

import { adminConnectionsQuery, type AdminConnection } from "../../api/admin";

export interface WorkspaceConnectionsAdminProps {
  workspaceId: string;
}

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, {
    dateStyle: "medium",
    timeStyle: "short",
  });
  return (iso: string) => format.format(new Date(iso));
}

/**
 * Admin → Workspaces → a workspace's connected accounts: each one's health,
 * and how its streams are syncing. Never anything read from the mailbox.
 * Shown by the workspaces module's workspace page.
 */
export function WorkspaceConnectionsAdmin({ workspaceId }: WorkspaceConnectionsAdminProps) {
  const { t } = useTranslation("connectors");
  const connections = useQuery(adminConnectionsQuery(workspaceId));

  return (
    <SettingsSection title={t("admin.title")}>
      {connections.isPending ? (
        <div role="status" aria-label={t("admin.loading")}>
          <SkeletonList rows={2} />
        </div>
      ) : connections.isError ? (
        <Banner
          tone="danger"
          title={t("admin.loadError")}
          action={
            <Button
              onClick={() => {
                void connections.refetch();
              }}
            >
              {t("admin.retry")}
            </Button>
          }
        />
      ) : connections.data.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("admin.none")}
        </Text>
      ) : (
        <FactList>
          {connections.data.map((connection) => (
            <ConnectionFact key={connection.id} connection={connection} />
          ))}
        </FactList>
      )}
    </SettingsSection>
  );
}

function ConnectionFact({ connection }: { connection: AdminConnection }) {
  const { t } = useTranslation("connectors");
  const formatDate = useDateFormat();
  // The newest sync of any stream (ISO timestamps sort as text).
  const lastSynced = connection.streams
    .map((stream) => stream.last_synced_at)
    .filter((at): at is string => typeof at === "string")
    .toSorted()
    .at(-1);
  const failures = Math.max(0, ...connection.streams.map((s) => s.consecutive_failures));
  const paused = connection.streams.some((stream) => stream.paused);
  const detail = [
    t(`admin.provider.${connection.provider}`),
    connection.capabilities.map((capability) => t(`admin.capability.${capability}`)).join(", "),
    lastSynced === undefined
      ? t("admin.neverSynced")
      : t("admin.lastSynced", { date: formatDate(lastSynced) }),
    paused ? t("admin.paused") : null,
    failures > 0 ? t("admin.failures", { count: failures }) : null,
  ]
    .filter((part) => part !== null && part !== "")
    .join(" · ");
  return (
    <Fact
      label={connection.address}
      status={connection.health === "ok" ? "ok" : "problem"}
      detail={detail}
    >
      {t(`admin.health.${connection.health}`)}
      {connection.health === "ok"
        ? null
        : ` (${t("admin.since", { date: formatDate(connection.health_changed_at) })})`}
    </Fact>
  );
}

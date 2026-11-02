import { Trans, useTranslation } from "react-i18next";

import { Banner, ButtonLink, SettingsPage, Text } from "~/lib/ui";
import type { Workspace } from "~/modules/workspaces";

import { useNotionConnections } from "../../api/queries";
import type { MailboxResult } from "../../model";
import { followIn } from "../../follow";
import { NotionCard } from "../NotionCard/NotionCard";

export interface ConnectionsPageProps {
  workspace: Workspace;
  /** What the browser came back from Notion with, said once at the top. */
  result?: MailboxResult | undefined;
  /** Follow an in-app address without a reload (the router's; absent: a normal link). */
  onNavigate?: ((href: string) => void) | undefined;
  /** The time "checked Notion …" counts from (tests pin it). */
  now?: Date | undefined;
  leave?: ((url: string) => void) | undefined;
}

/**
 * Settings → Connections: the apps that are not mail, connected to the
 * workspace by one member each. Today that is Notion (`NotionCard`); the page
 * ends with where mail and calendars live.
 */
export function ConnectionsPage({
  workspace,
  result,
  onNavigate,
  now,
  leave,
}: ConnectionsPageProps) {
  const { t } = useTranslation("connectors");
  const notion = useNotionConnections(workspace.id);
  const base = `/w/${workspace.id}`;
  const connected =
    result?.kind === "connected"
      ? notion.data?.find((connection) => connection.id === result.connection)
      : undefined;
  const mailboxes = followIn(onNavigate, `${base}/settings/mailboxes`);
  const calendar = followIn(onNavigate, `${base}/settings/calendar`);
  return (
    <SettingsPage
      label={t("connections.title")}
      title={t("connections.title")}
      lead={t("connections.lead")}
    >
      {result?.kind === "error" ? (
        <Banner tone="danger" role="alert" title={t(`errors.${result.error}.title`)}>
          {t(`errors.${result.error}.lead`)}
        </Banner>
      ) : null}
      {connected === undefined ? null : (
        <Banner
          role="status"
          icon="check"
          title={t("notion.connected.title", {
            name: connected.display_name ?? t("provider.notion"),
          })}
        >
          {t("notion.connected.lead")}
        </Banner>
      )}
      <NotionCard
        workspaceId={workspace.id}
        connections={notion.data}
        returnTo={`${base}/settings/connections`}
        importHref={`${base}/pages?import=notion`}
        onImport={(event) => {
          if (onNavigate === undefined || event.metaKey || event.ctrlKey || event.button !== 0) {
            return;
          }
          event.preventDefault();
          onNavigate(`${base}/pages?import=notion`);
        }}
        pagesHref={`${base}/pages`}
        onOpenPages={followIn(onNavigate, `${base}/pages`).onClick}
        now={now}
        leave={leave}
      />
      <Text tone="tertiary" size="sm">
        <Trans
          t={t}
          i18nKey="connections.elsewhere"
          components={{
            mail: (
              <ButtonLink variant="link" {...mailboxes}>
                …
              </ButtonLink>
            ),
            calendar: (
              <ButtonLink variant="link" {...calendar}>
                …
              </ButtonLink>
            ),
          }}
        />
      </Text>
    </SettingsPage>
  );
}

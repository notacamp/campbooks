import { useEffect, useRef, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Banner,
  Button,
  ButtonLink,
  InlineConfirm,
  List,
  ListRow,
  ProviderMark,
  SettingsEmpty,
  SettingsSection,
  StatusText,
  Text,
} from "~/lib/ui";

import { notionStartError, useStartNotion, type NotionStartError } from "../../api/notion";
import { useDisconnect, type Connection } from "../../api/queries";
import { healthTone } from "../../model";

/** Navigate the whole page (the consent leaves the app for Notion); tests pass a spy. */
const leavePage = (url: string) => {
  globalThis.location.assign(url);
};

export interface NotionCardProps {
  workspaceId: string;
  /** The connections to Notion the caller may see (undefined until known). */
  connections: readonly Connection[] | undefined;
  /** Where the browser comes back to after Notion (this page). */
  returnTo: string;
  /** Where "Import pages" goes: the Pages place with its import open. */
  importHref: string;
  /** Follow `importHref` without a reload. */
  onImport: (event: MouseEvent<HTMLElement>) => void;
  leave?: ((url: string) => void) | undefined;
}

/**
 * Settings → Connections → Notion: what connecting shares, Connect Notion
 * (members only; Notion opens and asks which pages to share), then the
 * connection with Import pages, Reconnect when it needs you, and Disconnect
 * (the pages stay in Campbooks and stop syncing; nothing is deleted in
 * Notion). An instance that does not offer Notion says so.
 */
export function NotionCard({
  workspaceId,
  connections,
  returnTo,
  importHref,
  onImport,
  leave = leavePage,
}: NotionCardProps) {
  const { t } = useTranslation("connectors");
  const start = useStartNotion(workspaceId);
  const [refused, setRefused] = useState<NotionStartError | undefined>();
  const [leaving, setLeaving] = useState(false);
  const [confirming, setConfirming] = useState<string | undefined>();
  const [disconnected, setDisconnected] = useState<string | undefined>();
  const doneRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (disconnected !== undefined) {
      doneRef.current?.focus();
    }
  }, [disconnected]);

  const begin = (connection?: string) => {
    setRefused(undefined);
    start.mutate(
      { return_to: returnTo, ...(connection === undefined ? {} : { connection }) },
      {
        onSuccess: (started) => {
          setLeaving(true);
          leave(started.authorize_url);
        },
        onError: (error) => {
          setRefused(notionStartError(error));
        },
      },
    );
  };
  const busy = start.isPending || leaving;
  const list = connections ?? [];
  const mine = list.find((connection) => connection.granted_by_you);

  return (
    <SettingsSection title={t("notion.title")} description={t("notion.description")}>
      {disconnected === undefined ? null : (
        <div ref={doneRef} tabIndex={-1}>
          <Banner role="status" icon="check" title={t("notion.disconnected.title")}>
            {t("notion.disconnected.lead")}
          </Banner>
        </div>
      )}
      {refused === undefined || refused === "unavailable" ? null : (
        <Banner tone="danger" role="alert" title={t(`notion.refusal.${refused}`)} />
      )}
      {list.length > 0 ? (
        <List label={t("notion.list")}>
          {list.map((connection) => (
            <NotionRow
              key={connection.id}
              workspaceId={workspaceId}
              connection={connection}
              busy={busy}
              confirming={confirming === connection.id}
              importHref={importHref}
              onImport={onImport}
              onReconnect={() => {
                begin(connection.id);
              }}
              onConfirm={(open) => {
                setConfirming(open ? connection.id : undefined);
              }}
              onDisconnected={() => {
                setConfirming(undefined);
                setDisconnected(connection.id);
              }}
            />
          ))}
        </List>
      ) : null}
      {mine !== undefined || connections === undefined ? null : refused === "unavailable" ? (
        <SettingsEmpty title={t("notion.unavailable.title")}>
          {t("notion.unavailable.lead")}
        </SettingsEmpty>
      ) : (
        <div className="flex flex-col gap-3">
          <ul className="flex list-disc flex-col gap-1 pl-5">
            {(["mine", "sync", "stop"] as const).map((point) => (
              <li key={point}>
                <Text tone="secondary">{t(`notion.bullets.${point}`)}</Text>
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              variant="secondary"
              icon="plus"
              disabled={busy}
              onClick={() => {
                begin();
              }}
            >
              {t("notion.connect")}
            </Button>
            <Text tone="tertiary" size="sm">
              {t("notion.next")}
            </Text>
          </div>
        </div>
      )}
    </SettingsSection>
  );
}

interface NotionRowProps {
  workspaceId: string;
  connection: Connection;
  busy: boolean;
  confirming: boolean;
  importHref: string;
  onImport: (event: MouseEvent<HTMLElement>) => void;
  onReconnect: () => void;
  onConfirm: (open: boolean) => void;
  onDisconnected: () => void;
}

function NotionRow({
  workspaceId,
  connection,
  busy,
  confirming,
  importHref,
  onImport,
  onReconnect,
  onConfirm,
  onDisconnected,
}: NotionRowProps) {
  const { t } = useTranslation("connectors");
  const disconnect = useDisconnect(workspaceId);
  const [failed, setFailed] = useState(false);
  const mine = connection.granted_by_you;
  const name = connection.display_name ?? t("provider.notion");
  const needsYou = connection.health === "needs_reauth";
  const health = (() => {
    switch (connection.health) {
      case "ok": {
        return t("notion.health.ok");
      }
      case "needs_reauth": {
        return mine ? t("health.needsYou") : t("health.needsAnother");
      }
      case "throttled": {
        return t("health.throttled", { provider: t("provider.notion") });
      }
      case "failing": {
        return t("health.failing");
      }
    }
  })();

  return (
    <ListRow
      leading={<ProviderMark name={t("provider.notion")} size="lg" />}
      title={name}
      meta={`${t("provider.notion")} · ${mine ? t("row.byYou") : t("row.byAnother")}`}
      trailing={
        <>
          {mine && !needsYou ? (
            <ButtonLink
              variant="secondary"
              href={importHref}
              onClick={onImport}
              aria-label={t("notion.importLabel", { name })}
            >
              {t("notion.import")}
            </ButtonLink>
          ) : null}
          {mine && needsYou ? (
            <Button
              variant="secondary"
              disabled={busy}
              aria-label={t("notion.reconnectLabel", { name })}
              onClick={onReconnect}
            >
              {t("row.reconnect")}
            </Button>
          ) : null}
          {mine ? (
            <Button
              variant="ghost"
              aria-label={t("notion.disconnectLabel", { name })}
              aria-expanded={confirming}
              onClick={() => {
                setFailed(false);
                onConfirm(true);
              }}
            >
              {t("row.disconnect")}
            </Button>
          ) : null}
        </>
      }
    >
      <StatusText tone={healthTone(connection.health)}>{health}</StatusText>
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("disconnect.confirm")}
          cancelLabel={t("disconnect.cancel")}
          busy={disconnect.isPending}
          onCancel={() => {
            onConfirm(false);
          }}
          onConfirm={() => {
            // Notion has no mail: what is kept is the pages, in Campbooks.
            disconnect.mutate(
              { connectionId: connection.id, mail: "keep" },
              {
                onSuccess: onDisconnected,
                onError: (error) => {
                  onConfirm(false);
                  setFailed(errorStatus(error) !== 404);
                },
              },
            );
          }}
        >
          <span>{t("notion.disconnect.question", { name })}</span>
          <br />
          <span>{t("notion.disconnect.lead")}</span>
        </InlineConfirm>
      ) : null}
      {failed ? (
        <Banner tone="danger" role="alert" title={t("notion.disconnect.failed", { name })} />
      ) : null}
    </ListRow>
  );
}

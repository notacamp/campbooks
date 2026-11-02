import { useEffect, useRef, useState, type MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus } from "~/lib/api";
import {
  Banner,
  Button,
  ButtonLink,
  Dialog,
  Fact,
  FactList,
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
import { agoLabel, healthTone, lastCheckedAt } from "../../model";

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
  /** The Pages place, for "Open in Pages". */
  pagesHref: string;
  /** Follow `pagesHref` without a reload. */
  onOpenPages: (event: MouseEvent<HTMLElement>) => void;
  /** The time "checked Notion …" counts from (tests pin it). */
  now?: Date | undefined;
  leave?: ((url: string) => void) | undefined;
}

/**
 * Settings → Connections → Notion: what connecting shares, Connect Notion
 * (members only; Notion opens and asks which pages to share), then the
 * connection with what it brings in, how it stays in step and what Notion
 * shares, and one row of actions: Import pages, Open in Pages, Reconnect when
 * it needs you, and Disconnect (asked in a dialog saying what stops and what
 * stays: the pages stay in Campbooks and stop syncing; nothing is deleted in
 * Notion). An instance that does not offer Notion says so.
 */
export function NotionCard({
  workspaceId,
  connections,
  returnTo,
  importHref,
  onImport,
  pagesHref,
  onOpenPages,
  now,
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
              pagesHref={pagesHref}
              onOpenPages={onOpenPages}
              now={now}
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
          <Text tone="secondary">{t("notion.notConnected")}</Text>
          {/* The indent sits on each item: the base reset zeroes a list's own padding. */}
          <ul className="flex list-disc flex-col gap-1">
            {(["mine", "sync", "stop"] as const).map((point) => (
              <li key={point} className="ml-5">
                <Text tone="secondary">{t(`notion.bullets.${point}`)}</Text>
              </li>
            ))}
          </ul>
          <div className="flex flex-wrap items-center gap-3">
            <Button
              variant="primary"
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
  pagesHref: string;
  onOpenPages: (event: MouseEvent<HTMLElement>) => void;
  now: Date | undefined;
  onReconnect: () => void;
  onConfirm: (open: boolean) => void;
  onDisconnected: () => void;
}

/** Phones get 44px targets; the actions sit two by two there. */
const TOUCH = "max-sm:min-h-11 max-sm:justify-center";

function NotionRow({
  workspaceId,
  connection,
  busy,
  confirming,
  importHref,
  onImport,
  pagesHref,
  onOpenPages,
  now,
  onReconnect,
  onConfirm,
  onDisconnected,
}: NotionRowProps) {
  const { t, i18n } = useTranslation("connectors");
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
  // A pull every few minutes, never "live": when a run last finished.
  const checkedAt = connection.health === "ok" ? lastCheckedAt(connection) : undefined;
  const status =
    checkedAt === undefined
      ? health
      : `${health} · ${t("notion.checked", {
          ago: agoLabel(checkedAt, now ?? new Date(), i18n.language),
        })}`;

  return (
    <ListRow
      leading={<ProviderMark name={t("provider.notion")} size="lg" />}
      title={t("notion.card.title", { name })}
      meta={mine ? t("row.byYou") : t("row.byAnother")}
    >
      <div className="flex flex-col gap-3">
        <StatusText tone={healthTone(connection.health)}>{status}</StatusText>
        <FactList>
          <Fact label={t("notion.facts.brings.label")}>{t("notion.facts.brings.value")}</Fact>
          <Fact label={t("notion.facts.sync.label")}>{t("notion.facts.sync.value")}</Fact>
          <Fact label={t("notion.facts.shares.label")}>{t("notion.facts.shares.value")}</Fact>
        </FactList>
        {mine ? (
          <div className="grid grid-cols-2 gap-2 sm:flex sm:flex-wrap sm:items-center">
            {needsYou ? null : (
              <ButtonLink
                variant="secondary"
                className={TOUCH}
                href={importHref}
                onClick={onImport}
                aria-label={t("notion.importLabel", { name })}
              >
                {t("notion.import")}
              </ButtonLink>
            )}
            <ButtonLink
              variant="secondary"
              className={TOUCH}
              href={pagesHref}
              onClick={onOpenPages}
            >
              {t("notion.openPages")}
            </ButtonLink>
            {needsYou ? (
              <Button
                variant="secondary"
                className={TOUCH}
                disabled={busy}
                aria-label={t("notion.reconnectLabel", { name })}
                onClick={onReconnect}
              >
                {t("row.reconnect")}
              </Button>
            ) : null}
            <Button
              variant="secondary"
              className={`${TOUCH} sm:ml-auto`}
              aria-label={t("notion.disconnectLabel", { name })}
              aria-haspopup="dialog"
              onClick={() => {
                setFailed(false);
                onConfirm(true);
              }}
            >
              {t("row.disconnect")}
            </Button>
          </div>
        ) : null}
      </div>
      <Dialog
        open={confirming}
        onClose={() => {
          onConfirm(false);
        }}
        title={t("notion.disconnect.title", { name })}
        visor={null}
        footer={
          <>
            <Button
              variant="secondary"
              keys="escape"
              onClick={() => {
                onConfirm(false);
              }}
            >
              {t("notion.disconnect.keep")}
            </Button>
            <Button
              variant="danger"
              keys="enter"
              data-autofocus=""
              disabled={disconnect.isPending}
              onClick={() => {
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
              {t("notion.disconnect.confirm")}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-4">
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            {[
              {
                id: "stops",
                title: t("notion.disconnect.stops.title"),
                points: [
                  t("notion.disconnect.stops.changes"),
                  t("notion.disconnect.stops.edits"),
                  t("notion.disconnect.stops.access"),
                ],
              },
              {
                id: "stays",
                title: t("notion.disconnect.stays.title"),
                points: [t("notion.disconnect.stays.pages"), t("notion.disconnect.stays.notion")],
              },
            ].map((group) => (
              <section key={group.id} className="flex flex-col gap-2">
                <h3>
                  <Text tone="secondary" size="sm">
                    {group.title}
                  </Text>
                </h3>
                <ul className="flex list-disc flex-col gap-1">
                  {group.points.map((point) => (
                    <li key={point} className="ml-5">
                      <Text>{point}</Text>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
          <Text tone="tertiary" size="sm">
            {t("notion.disconnect.again")}
          </Text>
        </div>
      </Dialog>
      {failed ? (
        <Banner tone="danger" role="alert" title={t("notion.disconnect.failed", { name })} />
      ) : null}
    </ListRow>
  );
}

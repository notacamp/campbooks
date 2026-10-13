import { useId, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, useSession } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  PaneHeader,
  ProviderMark,
  ReaderBody,
  ReaderPane,
  SkeletonList,
  Spacer,
  StatusText,
  Text,
} from "~/lib/ui";
import { useMembers, type Member, type Workspace } from "~/modules/workspaces";

import { useConnections, useDisconnect, type Connection } from "../../api/queries";
import type { MailboxConsent } from "../../consent";
import {
  firstChoice,
  healthTone,
  knownMailProvider,
  stepChoice,
  type MailboxResult,
} from "../../model";

export interface MailboxesPageProps {
  workspace: Workspace;
  /** The front door (`identity`'s, handed over by the app shell). */
  consent: MailboxConsent;
  /** What the front door came back with, if anything. */
  result?: MailboxResult | undefined;
}

/** A provider's name, in the user's language when it has one. */
function useProviderName() {
  const { t } = useTranslation("connectors");
  return (provider: string) => (knownMailProvider(provider) ? t(`provider.${provider}`) : provider);
}

/**
 * Settings → Mailboxes: the workspace's mailboxes you may see, how each is
 * doing, Reconnect for yours that need it, Disconnect (confirmed in place:
 * the mail stays, and so does signing in with that account), and Connect a
 * mailbox through the front door. ⇧C connects, J/K choose a mailbox, R
 * reconnects it, ⌫ asks to disconnect it.
 */
export function MailboxesPage({ workspace, consent, result }: MailboxesPageProps) {
  const { t } = useTranslation("connectors");
  const connections = useConnections(workspace.id);
  const members = useMembers(workspace.id);
  const session = useSession();
  const providerName = useProviderName();
  const connectGroup = useRef<HTMLDivElement>(null);
  const [chosen, setChosen] = useState<string | undefined>();
  const [confirming, setConfirming] = useState<string | undefined>();
  const [disconnected, setDisconnected] = useState<string | undefined>();

  const myEmail = session.data?.user?.email?.toLowerCase();
  const me = members.data?.find((member) => member.email?.toLowerCase() === myEmail);
  const mine = (connection: Connection) => me?.user_id === connection.granted_by;
  const list = connections.data ?? [];
  const current = list.some((connection) => connection.id === chosen)
    ? chosen
    : firstChoice(list, mine);
  const selected = list.find((connection) => connection.id === current);
  const seesAll = workspace.role === "owner" || workspace.role === "admin";

  useShortcut(
    "mailboxes.connect",
    () => {
      connectGroup.current?.querySelector("button")?.focus();
    },
    { enabled: (consent.providers?.length ?? 0) > 0 },
  );
  useShortcut(
    "list.next",
    () => {
      setChosen(stepChoice(list, current, 1));
    },
    { enabled: list.length > 0 },
  );
  useShortcut(
    "list.previous",
    () => {
      setChosen(stepChoice(list, current, -1));
    },
    { enabled: list.length > 0 },
  );
  useShortcut(
    "mailboxes.reconnect",
    () => {
      if (selected !== undefined) {
        consent.start({ purpose: "reconnect", connection: selected.id });
      }
    },
    {
      enabled: selected?.health === "needs_reauth" && mine(selected) && !consent.starting,
    },
  );
  useShortcut(
    "connectors.disconnect",
    () => {
      if (selected !== undefined) {
        setConfirming(selected.id);
      }
    },
    { enabled: selected !== undefined && confirming === undefined },
  );

  return (
    <ReaderPane label={t("mailboxes.title")}>
      <PaneHeader>
        <Heading level={1} size="pane">
          {t("mailboxes.title")}
        </Heading>
        <Spacer />
      </PaneHeader>
      <ReaderBody>
        <Text tone="secondary">{t("mailboxes.lead", { workspace: workspace.name })}</Text>
        <Outcome result={result} connections={connections.data} />
        {disconnected === undefined ? null : (
          <Banner
            role="status"
            icon="check"
            title={t("disconnect.done", { address: disconnected })}
          >
            {t("disconnect.doneLead")}
          </Banner>
        )}
        <Refusal consent={consent} />
        {connections.isPending ? (
          <div role="status" aria-label={t("mailboxes.loading")}>
            <SkeletonList rows={2} />
          </div>
        ) : connections.isError ? (
          <EmptyState
            title={t("mailboxes.loadError.title")}
            visor="thinking"
            visorSize="lg"
            action={
              <Button
                onClick={() => {
                  void connections.refetch();
                }}
              >
                {t("mailboxes.loadError.retry")}
              </Button>
            }
          >
            {t("mailboxes.loadError.lead")}
          </EmptyState>
        ) : list.length === 0 ? (
          <EmptyState title={t("mailboxes.empty.title")} visor="happy" visorSize="lg">
            {t("mailboxes.empty.lead")}
          </EmptyState>
        ) : (
          <List label={t("mailboxes.list")}>
            {list.map((connection) => (
              <MailboxRow
                key={connection.id}
                workspaceId={workspace.id}
                connection={connection}
                providerName={providerName(connection.provider)}
                grantor={members.data?.find((member) => member.user_id === connection.granted_by)}
                mine={mine(connection)}
                selected={connection.id === current}
                confirming={connection.id === confirming}
                consent={consent}
                onChoose={() => {
                  setChosen(connection.id);
                }}
                onConfirm={(open) => {
                  setChosen(connection.id);
                  setConfirming(open ? connection.id : undefined);
                }}
                onDisconnected={() => {
                  setConfirming(undefined);
                  setDisconnected(connection.address);
                }}
              />
            ))}
          </List>
        )}
        {seesAll ? null : (
          <Text tone="tertiary" size="sm">
            {t("mailboxes.onlyYours")}
          </Text>
        )}
        <ConnectSection groupRef={connectGroup} consent={consent} />
      </ReaderBody>
    </ReaderPane>
  );
}

/** What the last round trip did, said once at the top. */
function Outcome({
  result,
  connections,
}: {
  result: MailboxResult | undefined;
  connections: readonly Connection[] | undefined;
}) {
  const { t } = useTranslation("connectors");
  const providerName = useProviderName();
  if (result === undefined) {
    return null;
  }
  if (result.kind === "error") {
    return (
      <Banner tone="danger" role="alert" title={t(`errors.${result.error}.title`)}>
        {t(`errors.${result.error}.lead`)}
      </Banner>
    );
  }
  const connected = connections?.find((connection) => connection.id === result.connection);
  return (
    <Banner
      role="status"
      icon="check"
      title={
        connected === undefined
          ? t("connected.title")
          : t("connected.named", {
              provider: providerName(connected.provider),
              address: connected.address,
            })
      }
    >
      {t("connected.lead")}
    </Banner>
  );
}

/** Why starting was refused: "Sign in again first", or a sentence. */
function Refusal({ consent }: { consent: MailboxConsent }) {
  const { t } = useTranslation("connectors");
  if (consent.refusal === undefined) {
    return null;
  }
  if (consent.refusal === "step_up") {
    return (
      <Banner
        role="alert"
        icon="key"
        title={t("stepUp.title")}
        action={
          <Button variant="secondary" onClick={consent.signInAgain}>
            {t("stepUp.action")}
          </Button>
        }
      >
        {t("stepUp.lead")}
      </Banner>
    );
  }
  return <Banner tone="danger" role="alert" title={t(`refusal.${consent.refusal}`)} />;
}

interface MailboxRowProps {
  workspaceId: string;
  connection: Connection;
  providerName: string;
  /** The member who connected it, when the list has them. */
  grantor: Member | undefined;
  /** You connected it (only you can reconnect it). */
  mine: boolean;
  /** The row the keys act on. */
  selected: boolean;
  /** Its Disconnect confirmation is open. */
  confirming: boolean;
  consent: MailboxConsent;
  onChoose: () => void;
  onConfirm: (open: boolean) => void;
  onDisconnected: () => void;
}

function MailboxRow({
  workspaceId,
  connection,
  providerName,
  grantor,
  mine,
  selected,
  confirming,
  consent,
  onChoose,
  onConfirm,
  onDisconnected,
}: MailboxRowProps) {
  const { t } = useTranslation("connectors");
  const disconnect = useDisconnect(workspaceId);
  const [failed, setFailed] = useState(false);
  const { address } = connection;
  const grantorName = grantor?.display_name ?? grantor?.email ?? undefined;
  const by = mine
    ? t("row.byYou")
    : grantorName === undefined
      ? t("row.byAnother")
      : t("row.by", { name: grantorName });
  const reconnect = connection.health === "needs_reauth" && mine;

  const health = (() => {
    switch (connection.health) {
      case "ok": {
        return t("health.ok");
      }
      case "needs_reauth": {
        return mine || grantorName === undefined
          ? t("health.needsYou")
          : t("health.needsThem", { name: grantorName });
      }
      case "throttled": {
        return t("health.throttled", { provider: providerName });
      }
      case "failing": {
        return t("health.failing");
      }
    }
  })();

  return (
    <ListRow
      selected={selected}
      leading={<ProviderMark name={providerName} size="lg" />}
      title={address}
      meta={`${providerName} · ${by}`}
      trailing={
        <>
          {reconnect ? (
            <Button
              variant="secondary"
              disabled={consent.starting}
              shortcut={selected ? "mailboxes.reconnect" : undefined}
              aria-label={t("row.reconnectLabel", { address })}
              onClick={() => {
                onChoose();
                consent.start({ purpose: "reconnect", connection: connection.id });
              }}
            >
              {t("row.reconnect")}
            </Button>
          ) : null}
          <Button
            variant="ghost"
            shortcut={selected ? "connectors.disconnect" : undefined}
            aria-label={t("row.disconnectLabel", { address })}
            aria-expanded={confirming}
            onClick={() => {
              setFailed(false);
              onConfirm(true);
            }}
          >
            {t("row.disconnect")}
          </Button>
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
            disconnect.mutate(connection.id, {
              onSuccess: onDisconnected,
              onError: (error) => {
                onConfirm(false);
                // Already gone (or never yours to see): the list catches up.
                if (errorStatus(error) === 404) {
                  onDisconnected();
                } else {
                  setFailed(true);
                }
              },
            });
          }}
        >
          {t("disconnect.question", { address, provider: providerName })}
        </InlineConfirm>
      ) : null}
      {failed ? (
        <Banner tone="danger" role="alert" title={t("disconnect.failed", { address })} />
      ) : null}
    </ListRow>
  );
}

function ConnectSection({
  groupRef,
  consent,
}: {
  groupRef: RefObject<HTMLDivElement | null>;
  consent: MailboxConsent;
}) {
  const { t } = useTranslation("connectors");
  const providerName = useProviderName();
  const titleId = useId();
  const providers = consent.providers;
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("connect.title")}
      </Heading>
      {providers === undefined ? (
        consent.providersFailed ? (
          <Text tone="tertiary" size="sm">
            {t("connect.loadError")}
          </Text>
        ) : (
          <div role="status" aria-label={t("connect.loading")}>
            <SkeletonList rows={1} />
          </div>
        )
      ) : providers.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("connect.none")}
        </Text>
      ) : (
        <>
          <Text tone="tertiary" size="sm">
            {t("connect.lead")}
          </Text>
          <div
            ref={groupRef}
            className="flex flex-col gap-2 sm:flex-row sm:flex-wrap"
            role="group"
            aria-labelledby={titleId}
          >
            {providers.map((provider, index) => (
              <Button
                key={provider}
                variant={index === 0 ? "primary" : "secondary"}
                icon="plus"
                disabled={consent.starting}
                shortcut={index === 0 ? "mailboxes.connect" : undefined}
                onClick={() => {
                  consent.start({ purpose: "connect", provider });
                }}
              >
                {t("connect.provider", { provider: providerName(provider) })}
              </Button>
            ))}
          </div>
        </>
      )}
    </section>
  );
}

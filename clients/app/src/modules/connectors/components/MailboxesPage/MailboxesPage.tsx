import { useEffect, useId, useRef, useState, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Checkbox,
  ChoiceCards,
  EmptyState,
  InlineConfirm,
  List,
  ListRow,
  ProviderMark,
  SettingsEmpty,
  SettingsPage,
  SettingsSection,
  SkeletonList,
  StatusText,
  Text,
} from "~/lib/ui";
import { useMembers, type Member, type Workspace } from "~/modules/workspaces";

import { useConnections, useDisconnect, type Connection, type MailChoice } from "../../api/queries";
import type { MailboxConsent, WayInFate } from "../../consent";
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
 * doing, Reconnect and Disconnect for the ones you connected (Disconnect
 * confirmed in place: delete its mail here or keep its history read-only;
 * the mail at the provider is never touched; a way in added only by
 * connecting it goes with it), and Connect a mailbox through the front door
 * (also as a way in, or not). ⇧C connects, J/K choose a mailbox, R
 * reconnects it, ⌫ asks to disconnect it.
 */
export function MailboxesPage({ workspace, consent, result }: MailboxesPageProps) {
  const { t } = useTranslation("connectors");
  const connections = useConnections(workspace.id);
  const members = useMembers(workspace.id);
  const providerName = useProviderName();
  const connectGroup = useRef<HTMLDivElement>(null);
  const doneRef = useRef<HTMLDivElement>(null);
  const [chosen, setChosen] = useState<string | undefined>();
  const [confirming, setConfirming] = useState<string | undefined>();
  const [disconnected, setDisconnected] = useState<
    { address: string; provider: string; mail: MailChoice } | undefined
  >();
  // A disconnect that found nothing: said here, since its row goes with the
  // list's next answer.
  const [gone, setGone] = useState<string | undefined>();

  useEffect(() => {
    if (disconnected !== undefined || gone !== undefined) {
      doneRef.current?.focus();
    }
  }, [disconnected, gone]);

  // The server marks the caller's own connections (by id, never by email).
  const mine = (connection: Connection) => connection.granted_by_you;
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
    // Only who connected a mailbox may disconnect it.
    { enabled: selected !== undefined && mine(selected) && confirming === undefined },
  );

  return (
    <SettingsPage
      label={t("mailboxes.title")}
      title={t("mailboxes.title")}
      lead={t("mailboxes.lead", { workspace: workspace.name })}
    >
      {/* Said once: a disconnect afterwards replaces it. */}
      {disconnected === undefined ? (
        <Outcome result={result} connections={connections.data} />
      ) : null}
      {gone === undefined ? null : (
        <div ref={doneRef} tabIndex={-1}>
          <Banner
            tone="danger"
            role="alert"
            title={t("disconnect.errors.gone", { address: gone })}
          />
        </div>
      )}
      {disconnected === undefined ? null : (
        // Focus lands here once the row it was in is gone.
        <div ref={doneRef} tabIndex={-1}>
          <Banner
            role="status"
            icon="check"
            title={t("disconnect.done", { address: disconnected.address })}
          >
            {disconnected.mail === "keep"
              ? t("disconnect.doneKept", { provider: disconnected.provider })
              : t("disconnect.doneDeleted", { provider: disconnected.provider })}
          </Banner>
        </div>
      )}
      <Refusal consent={consent} />
      <SettingsSection
        title={t("mailboxes.list")}
        count={connections.data?.length}
        description={seesAll ? undefined : t("mailboxes.onlyYours")}
      >
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
          <SettingsEmpty title={t("mailboxes.empty.title")}>
            {t("mailboxes.empty.lead")}
          </SettingsEmpty>
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
                onGone={() => {
                  setConfirming(undefined);
                  setDisconnected(undefined);
                  setGone(connection.address);
                }}
                onDisconnected={(mail) => {
                  setConfirming(undefined);
                  setGone(undefined);
                  setDisconnected({
                    address: connection.address,
                    provider: providerName(connection.provider),
                    mail,
                  });
                }}
              />
            ))}
          </List>
        )}
      </SettingsSection>
      <ConnectSection groupRef={connectGroup} consent={consent} />
    </SettingsPage>
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
  /** You connected it (only you can reconnect or disconnect it). */
  mine: boolean;
  /** The row the keys act on. */
  selected: boolean;
  /** Its Disconnect confirmation is open. */
  confirming: boolean;
  consent: MailboxConsent;
  onChoose: () => void;
  onConfirm: (open: boolean) => void;
  onDisconnected: (mail: MailChoice) => void;
  /** The disconnect found nothing (404): the page says so. */
  onGone: () => void;
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
  onGone,
}: MailboxRowProps) {
  const { t } = useTranslation("connectors");
  const disconnect = useDisconnect(workspaceId);
  const [failed, setFailed] = useState<"notYours" | "failed" | undefined>();
  const [mail, setMail] = useState<MailChoice>("delete");
  const disconnectButton = useRef<HTMLButtonElement>(null);
  // Back on Disconnect when the confirmation closes and the row stays
  // (cancelled or refused), so focus never falls to the page.
  const wasConfirming = useRef(false);
  useEffect(() => {
    if (wasConfirming.current && !confirming) {
      disconnectButton.current?.focus();
    }
    wasConfirming.current = confirming;
  }, [confirming]);
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
        return mine
          ? t("health.needsYou")
          : grantorName === undefined
            ? t("health.needsAnother")
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
          {mine ? (
            <Button
              ref={disconnectButton}
              variant="ghost"
              shortcut={selected ? "connectors.disconnect" : undefined}
              aria-label={t("row.disconnectLabel", { address })}
              aria-expanded={confirming}
              onClick={() => {
                setFailed(undefined);
                setMail("delete");
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
            disconnect.mutate(
              { connectionId: connection.id, mail },
              {
                onSuccess: () => {
                  onDisconnected(mail);
                },
                onError: (error) => {
                  onConfirm(false);
                  // Never a silent success: a 404 is said as one, by the
                  // page (this row leaves with the list's next answer).
                  if (errorStatus(error) === 404) {
                    onGone();
                    return;
                  }
                  setFailed(problemCode(error) === "disconnect_owner_only" ? "notYours" : "failed");
                },
              },
            );
          }}
        >
          <DisconnectChoice
            address={address}
            provider={providerName}
            wayIn={consent.wayInFate(connection.id)}
            mail={mail}
            onMail={setMail}
          />
        </InlineConfirm>
      ) : null}
      {failed === undefined ? null : (
        <Banner tone="danger" role="alert" title={t(`disconnect.errors.${failed}`, { address })} />
      )}
    </ListRow>
  );
}

/**
 * What Disconnect does, said before it happens: syncing stops, what becomes
 * of the mail here (delete by default, or keep its history read-only), that
 * the mail stays at the provider, and what becomes of the way in.
 */
function DisconnectChoice({
  address,
  provider,
  wayIn,
  mail,
  onMail,
}: {
  address: string;
  provider: string;
  wayIn: WayInFate | undefined;
  mail: MailChoice;
  onMail: (mail: MailChoice) => void;
}) {
  const { t } = useTranslation("connectors");
  return (
    <div className="flex flex-col gap-3">
      <span>{t("disconnect.question", { address, provider })}</span>
      <ChoiceCards
        label={t("disconnect.choice")}
        value={mail}
        onChange={onMail}
        options={[
          {
            value: "delete",
            title: t("disconnect.delete.title"),
            description: t("disconnect.delete.lead"),
          },
          {
            value: "keep",
            title: t("disconnect.keep.title"),
            description: t("disconnect.keep.lead", { provider }),
          },
        ]}
      />
      <span>
        {t("disconnect.atProvider", { provider })}
        {wayIn === undefined ? null : <> {t(`disconnect.wayIn.${wayIn}`)}</>}
      </span>
    </div>
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
  const [signIn, setSignIn] = useState(true);
  return (
    <SettingsSection
      title={t("connect.title")}
      description={providers !== undefined && providers.length > 0 ? t("connect.lead") : undefined}
      titleId={titleId}
    >
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
          <Checkbox
            label={t("connect.signIn")}
            hint={t("connect.signInHint")}
            checked={signIn}
            onChange={(event) => {
              setSignIn(event.currentTarget.checked);
            }}
          />
          <div
            ref={groupRef}
            className="flex flex-col gap-2 sm:flex-row sm:flex-wrap"
            role="group"
            aria-labelledby={titleId}
          >
            {providers.map((provider, index) => (
              <Button
                key={provider}
                variant="secondary"
                icon="plus"
                disabled={consent.starting}
                shortcut={index === 0 ? "mailboxes.connect" : undefined}
                onClick={() => {
                  consent.start({ purpose: "connect", provider, signIn });
                }}
              >
                {t("connect.provider", { provider: providerName(provider) })}
              </Button>
            ))}
          </div>
        </>
      )}
    </SettingsSection>
  );
}

import { useId, useRef, useState, type ComponentType, type RefObject } from "react";
import { useTranslation } from "react-i18next";

import { problemCode, signOutAndLeave } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  EmptyState,
  Heading,
  Icon,
  InlineConfirm,
  List,
  ListRow,
  PaneHeader,
  ReaderBody,
  ReaderPane,
  SkeletonList,
  Spacer,
  Text,
} from "~/lib/ui";
import { useConnections } from "~/modules/connectors";

import {
  usePasskeys,
  useRemoveWayIn,
  useSignOutEverywhere,
  useStartLink,
  useWaysIn,
  type StartLink,
  type WayIn,
} from "../../api/queries";
import {
  addedRecently,
  knownProvider,
  leavePage,
  stepUpUrl,
  type LinkResult,
  type Provider,
} from "../../model";

export interface WaysInPageProps {
  /** The open workspace: where a mailbox to reconnect lives. */
  workspaceId: string;
  /** What the front door came back with, if anything. */
  result?: LinkResult | undefined;
  /** This page's own path, to come back to after a provider or a fresh sign-in. */
  returnTo: string;
  /**
   * More ways in, each its own section below the providers (passkeys). The
   * module passes {@link WAY_IN_SECTIONS}.
   */
  sections?: readonly ComponentType[] | undefined;
  /** Leaves the app for a provider or the sign-in (tests and stories pass a spy). */
  leave?: ((url: string) => void) | undefined;
}

/** A provider's add-button label key, typed for `t()`. */
const ADD = {
  google: "waysIn.add.google",
  microsoft: "waysIn.add.microsoft",
  zoho: "waysIn.add.zoho",
} as const satisfies Record<Provider, string>;

function useDateFormat() {
  const { i18n } = useTranslation();
  const format = new Intl.DateTimeFormat(i18n.language, { dateStyle: "medium" });
  return (iso: string) => format.format(new Date(iso));
}

/** Whether `error` asks for a fresh sign-in first. */
const needsStepUp = (error: unknown) => problemCode(error) === "step_up_required";

/**
 * Settings → Ways to sign in: every provider account you sign in with (and
 * Remove, confirmed in place, never the last), Add another, Reconnect a
 * mailbox of yours here that stopped syncing, and the sections other ways in
 * bring (passkeys). Changing how you sign in needs a sign-in in the last ten
 * minutes; the page says so and offers one. A way in added in the last week
 * stands out (it may be a stranger's), and Sign out everywhere ends every
 * sign-in of the account.
 */
export function WaysInPage({
  workspaceId,
  result,
  returnTo,
  sections = [],
  leave = leavePage,
}: WaysInPageProps) {
  const { t } = useTranslation("identity");
  const ways = useWaysIn();
  const start = useStartLink();
  const [stepUp, setStepUp] = useState(false);
  const addGroup = useRef<HTMLDivElement>(null);

  const go = (body: StartLink) => {
    setStepUp(false);
    start.mutate(
      { ...body, return_to: returnTo },
      {
        onSuccess: (url) => {
          leave(url);
        },
        onError: (failure) => {
          setStepUp(needsStepUp(failure));
        },
      },
    );
  };

  useShortcut(
    "identity.start_link",
    () => {
      addGroup.current?.querySelector("button")?.focus();
    },
    { enabled: ways.isSuccess },
  );

  return (
    <ReaderPane label={t("waysIn.title")}>
      <PaneHeader>
        <Heading level={1} size="pane">
          {t("waysIn.title")}
        </Heading>
        <Spacer />
      </PaneHeader>
      <ReaderBody>
        <Text tone="secondary">{t("waysIn.lead")}</Text>
        <Outcome result={result} />
        <RecentlyAdded ways={ways.data?.items} />
        {stepUp ? <StepUp returnTo={returnTo} leave={leave} /> : null}
        {start.isError && !stepUp ? (
          <Banner tone="danger" role="alert" title={t("waysIn.startFailed")} />
        ) : null}
        {ways.isPending ? (
          <div role="status" aria-label={t("waysIn.loading")}>
            <SkeletonList rows={2} />
          </div>
        ) : ways.isError ? (
          <EmptyState
            title={t("waysIn.loadError.title")}
            visor="thinking"
            visorSize="lg"
            action={
              <Button
                onClick={() => {
                  void ways.refetch();
                }}
              >
                {t("waysIn.loadError.retry")}
              </Button>
            }
          >
            {t("waysIn.loadError.lead")}
          </EmptyState>
        ) : (
          <>
            <ProviderWays
              items={ways.data.items}
              workspaceId={workspaceId}
              busy={start.isPending}
              onReconnect={(connection) => {
                go({ purpose: "reconnect", workspace_id: workspaceId, connection_id: connection });
              }}
              onStepUp={() => {
                setStepUp(true);
              }}
            />
            <AddWayIn
              groupRef={addGroup}
              providers={ways.data.providers.filter(knownProvider)}
              busy={start.isPending}
              onAdd={(provider) => {
                go({ purpose: "link", provider });
              }}
            />
          </>
        )}
        {sections.map((Section, index) => (
          <Section key={index} />
        ))}
        <SignOutEverywhere leave={leave} />
      </ReaderBody>
    </ReaderPane>
  );
}

/** What the last round trip did, said once at the top. */
function Outcome({ result }: { result: LinkResult | undefined }) {
  const { t } = useTranslation("identity");
  if (result === undefined) {
    return null;
  }
  if (result.kind === "error") {
    return (
      <Banner tone="danger" role="alert" title={t(`waysIn.errors.${result.error}.title`)}>
        {t(`waysIn.errors.${result.error}.lead`)}
      </Banner>
    );
  }
  return (
    <Banner
      role="status"
      icon="check"
      title={
        result.kind === "linked"
          ? t("waysIn.linked", { provider: t(`provider.${result.provider}`) })
          : t("waysIn.connected")
      }
    />
  );
}

/**
 * Says so when a way to sign in was added in the last week (other than the
 * one you signed up with): if it wasn't you, what to do.
 */
function RecentlyAdded({ ways }: { ways: readonly WayIn[] | undefined }) {
  const { t } = useTranslation("identity");
  const passkeys = usePasskeys();
  const recent =
    (ways ?? []).some((way) => way.linked_via !== "sign_up" && addedRecently(way.linked_at)) ||
    (passkeys.data?.items ?? []).some(
      (passkey) => !passkey.signed_up_with && addedRecently(passkey.created_at),
    );
  if (!recent) {
    return null;
  }
  return (
    <Banner role="status" icon="key" title={t("waysIn.recent.title")}>
      {t("waysIn.recent.lead")}
    </Banner>
  );
}

/**
 * Sign out everywhere: every sign-in of the account ends (this browser's,
 * other devices', every app's), confirmed in place; then this browser leaves
 * for the sign-in.
 */
function SignOutEverywhere({ leave }: { leave: (url: string) => void }) {
  const { t } = useTranslation("identity");
  const titleId = useId();
  const end = useSignOutEverywhere();
  const [confirming, setConfirming] = useState(false);
  const [failed, setFailed] = useState(false);
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("waysIn.everywhere.title")}
      </Heading>
      <Text tone="tertiary" size="sm">
        {t("waysIn.everywhere.lead")}
      </Text>
      {failed ? <Banner tone="danger" role="alert" title={t("waysIn.everywhere.failed")} /> : null}
      <div>
        <Button
          variant="danger"
          icon="sign-out"
          aria-expanded={confirming}
          onClick={() => {
            setFailed(false);
            setConfirming(true);
          }}
        >
          {t("waysIn.everywhere.action")}
        </Button>
      </div>
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("waysIn.everywhere.confirm")}
          cancelLabel={t("waysIn.cancel")}
          busy={end.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            setFailed(false);
            end.mutate(undefined, {
              onSuccess: () => {
                // This browser too: its session ends now, then the sign-in.
                signOutAndLeave(leave).catch(() => {
                  leave("/");
                });
              },
              onError: () => {
                setConfirming(false);
                setFailed(true);
              },
            });
          }}
        >
          {t("waysIn.everywhere.confirmLead")}
        </InlineConfirm>
      ) : null}
    </section>
  );
}

/** "Sign in again first", with the way to do it. */
function StepUp({ returnTo, leave }: { returnTo: string; leave: (url: string) => void }) {
  const { t } = useTranslation("identity");
  return (
    <Banner
      role="alert"
      icon="key"
      title={t("waysIn.stepUp.title")}
      action={
        <Button
          variant="secondary"
          onClick={() => {
            leave(stepUpUrl(returnTo));
          }}
        >
          {t("waysIn.stepUp.action")}
        </Button>
      }
    >
      {t("waysIn.stepUp.lead")}
    </Banner>
  );
}

function ProviderWays({
  items,
  workspaceId,
  busy,
  onReconnect,
  onStepUp,
}: {
  items: readonly WayIn[];
  workspaceId: string;
  busy: boolean;
  onReconnect: (connection: string) => void;
  onStepUp: () => void;
}) {
  const { t } = useTranslation("identity");
  const connections = useConnections(workspaceId);
  // A mailbox of yours here that stopped syncing, by its address: every
  // connected mailbox is a way in, so it is one of these rows. The server
  // checks it is yours when you reconnect.
  const needsYou = (way: WayIn) =>
    connections.data?.find(
      (connection) =>
        connection.health === "needs_reauth" &&
        connection.provider === way.provider &&
        connection.address.toLowerCase() === way.email?.toLowerCase(),
    );
  return (
    <List label={t("waysIn.list")}>
      {items.map((way) => (
        <WayInRow
          key={way.id}
          way={way}
          only={items.length === 1}
          reconnect={needsYou(way)?.id}
          busy={busy}
          onReconnect={onReconnect}
          onStepUp={onStepUp}
        />
      ))}
    </List>
  );
}

function WayInRow({
  way,
  only,
  reconnect,
  busy,
  onReconnect,
  onStepUp,
}: {
  way: WayIn;
  /** The only provider way in listed (passkeys may still count). */
  only: boolean;
  /** The connection of this account here that needs reconnecting. */
  reconnect: string | undefined;
  busy: boolean;
  onReconnect: (connection: string) => void;
  onStepUp: () => void;
}) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const remove = useRemoveWayIn();
  const [confirming, setConfirming] = useState(false);
  const [error, setError] = useState<"lastWayIn" | "removeFailed" | undefined>();
  const provider = knownProvider(way.provider) ? t(`provider.${way.provider}`) : way.provider;
  const address = way.email ?? provider;
  const meta = [
    way.email,
    t("waysIn.added", { date: formatDate(way.linked_at) }),
    way.last_used_at === null || way.last_used_at === undefined
      ? null
      : t("waysIn.lastUsed", { date: formatDate(way.last_used_at) }),
  ]
    .filter((part) => part !== null && part !== undefined)
    .join(" · ");

  return (
    <ListRow
      leading={<Icon name={reconnect === undefined ? "key" : "warning"} size="lg" />}
      title={provider}
      badge={
        way.linked_via === "sign_up"
          ? t("waysIn.signedUpWith")
          : addedRecently(way.linked_at)
            ? t("waysIn.new")
            : undefined
      }
      meta={meta}
      trailing={
        <>
          {reconnect === undefined ? null : (
            <Button
              variant="secondary"
              disabled={busy}
              aria-label={t("waysIn.reconnectLabel", { address })}
              onClick={() => {
                onReconnect(reconnect);
              }}
            >
              {t("waysIn.reconnect")}
            </Button>
          )}
          <Button
            variant="ghost"
            aria-label={t("waysIn.removeLabel", { provider, address })}
            aria-expanded={confirming}
            onClick={() => {
              setError(undefined);
              setConfirming(true);
            }}
          >
            {t("waysIn.remove")}
          </Button>
        </>
      }
    >
      {reconnect === undefined ? null : (
        <Text tone="danger" size="sm">
          {t("waysIn.needsReconnect")}
        </Text>
      )}
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("waysIn.confirm")}
          cancelLabel={t("waysIn.cancel")}
          busy={remove.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            remove.mutate(way.id, {
              onError: (failure) => {
                setConfirming(false);
                if (needsStepUp(failure)) {
                  onStepUp();
                } else {
                  setError(problemCode(failure) === "last_way_in" ? "lastWayIn" : "removeFailed");
                }
              },
            });
          }}
        >
          {only
            ? t("waysIn.confirmRemoveOnly", { provider, address })
            : t("waysIn.confirmRemove", { provider, address })}
        </InlineConfirm>
      ) : null}
      {error === undefined ? null : (
        <Banner tone="danger" role="alert" title={t(`waysIn.${error}.title`)}>
          {t(`waysIn.${error}.lead`)}
        </Banner>
      )}
    </ListRow>
  );
}

function AddWayIn({
  groupRef,
  providers,
  busy,
  onAdd,
}: {
  groupRef: RefObject<HTMLDivElement | null>;
  providers: readonly Provider[];
  busy: boolean;
  onAdd: (provider: Provider) => void;
}) {
  const { t } = useTranslation("identity");
  const titleId = useId();
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={2} size="section" id={titleId}>
        {t("waysIn.addTitle")}
      </Heading>
      <Text tone="tertiary" size="sm">
        {providers.length === 0 ? t("waysIn.addNone") : t("waysIn.addLead")}
      </Text>
      {providers.length === 0 ? null : (
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
              disabled={busy}
              shortcut={index === 0 ? "identity.start_link" : undefined}
              onClick={() => {
                onAdd(provider);
              }}
            >
              {t(ADD[provider])}
            </Button>
          ))}
        </div>
      )}
    </section>
  );
}

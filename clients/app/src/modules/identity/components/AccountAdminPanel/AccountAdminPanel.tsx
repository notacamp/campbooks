import { useQuery } from "@tanstack/react-query";
import { useId, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import {
  Banner,
  Button,
  Fact,
  FactList,
  Heading,
  SettingsSection,
  InlineConfirm,
  List,
  ListRow,
  SkeletonList,
  Text,
} from "~/lib/ui";

import { adminAccountQuery, useSuspendAccount, type AdminAccount } from "../../api/admin";

export interface AccountAdminPanelProps {
  /** The identity account (a token `sub`). */
  accountId: string;
  /** How the page names the person ("Ana Duarte"). */
  name: string;
}

const PROVIDERS = ["google", "microsoft", "zoho"] as const;

/** Whether `name` is a provider the sign-in screens name. */
function isProvider(name: string): name is (typeof PROVIDERS)[number] {
  return (PROVIDERS as readonly string[]).includes(name);
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
 * Admin → People → a person's account: whether they can sign in, how, and
 * where they are signed in now; and suspending or unsuspending it. Shown
 * by the workspaces module's person page.
 */
export function AccountAdminPanel({ accountId, name }: AccountAdminPanelProps) {
  const { t } = useTranslation("identity");
  const account = useQuery(adminAccountQuery(accountId));

  return (
    <SettingsSection title={t("admin.title")}>
      {account.isPending ? (
        <div role="status" aria-label={t("admin.loading")}>
          <SkeletonList rows={3} />
        </div>
      ) : account.isError ? (
        errorStatus(account.error) === 404 ? (
          <Text tone="tertiary" size="sm">
            {t("admin.noAccount")}
          </Text>
        ) : (
          <Banner
            tone="danger"
            title={t("admin.loadError")}
            action={
              <Button
                onClick={() => {
                  void account.refetch();
                }}
              >
                {t("admin.retry")}
              </Button>
            }
          />
        )
      ) : (
        <>
          <AccountFacts account={account.data} />
          <WaysIn account={account.data} />
          <Suspension account={account.data} name={name} />
        </>
      )}
    </SettingsSection>
  );
}

function AccountFacts({ account }: { account: AdminAccount }) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const suspendedAt = account.suspended_at ?? undefined;
  const lastSignIn = account.sessions.last_sign_in_at ?? undefined;
  return (
    <FactList>
      <Fact
        label={t("admin.status")}
        status={suspendedAt === undefined ? "ok" : "problem"}
        detail={
          suspendedAt === undefined
            ? undefined
            : t("admin.suspendedSince", { date: formatDate(suspendedAt) })
        }
      >
        {suspendedAt === undefined ? t("admin.active") : t("admin.suspended")}
      </Fact>
      <Fact
        label={t("admin.primaryEmail")}
        detail={account.primary_email_verified ? t("admin.verified") : t("admin.unverified")}
      >
        {account.primary_email ?? t("admin.none")}
      </Fact>
      <Fact label={t("admin.created")}>{formatDate(account.created_at)}</Fact>
      <Fact label={t("admin.lastSignIn")}>
        {lastSignIn === undefined ? t("admin.never") : formatDate(lastSignIn)}
      </Fact>
      <Fact label={t("admin.sessions")}>
        {t("admin.sessionCount", { count: account.sessions.active })}
      </Fact>
      <Fact label={t("admin.passkeys")}>
        {t("admin.passkeyCount", { count: account.passkeys })}
      </Fact>
    </FactList>
  );
}

function WaysIn({ account }: { account: AdminAccount }) {
  const { t } = useTranslation("identity");
  const formatDate = useDateFormat();
  const titleId = useId();
  return (
    <section className="flex flex-col gap-3" aria-labelledby={titleId}>
      <Heading level={3} size="section" id={titleId}>
        {t("admin.waysIn")}
      </Heading>
      {account.ways_in.length === 0 ? (
        <Text tone="tertiary" size="sm">
          {t("admin.noWaysIn")}
        </Text>
      ) : (
        <List label={t("admin.waysIn")}>
          {account.ways_in.map((way) => (
            <ListRow
              key={`${way.provider}-${way.linked_at}`}
              title={isProvider(way.provider) ? t(`provider.${way.provider}`) : way.provider}
              meta={[
                way.email,
                t("waysIn.added", { date: formatDate(way.linked_at) }),
                way.last_used_at === null || way.last_used_at === undefined
                  ? null
                  : t("waysIn.lastUsed", { date: formatDate(way.last_used_at) }),
              ]
                .filter((part) => part !== null && part !== undefined)
                .join(" · ")}
            />
          ))}
        </List>
      )}
    </section>
  );
}

function Suspension({ account, name }: { account: AdminAccount; name: string }) {
  const { t } = useTranslation("identity");
  const change = useSuspendAccount(account.id);
  const [confirming, setConfirming] = useState(false);
  const suspended = account.suspended_at !== null && account.suspended_at !== undefined;
  const refused = problemCode(change.error) === "account_is_admin";

  return (
    <div className="flex flex-col gap-3">
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("admin.suspend")}
          cancelLabel={t("admin.cancel")}
          busy={change.isPending}
          onCancel={() => {
            setConfirming(false);
          }}
          onConfirm={() => {
            change.mutate(true, {
              onSettled: () => {
                setConfirming(false);
              },
            });
          }}
        >
          {t("admin.confirmSuspend", { name })}
        </InlineConfirm>
      ) : (
        <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
          {suspended ? (
            <Button
              disabled={change.isPending}
              onClick={() => {
                change.mutate(false);
              }}
            >
              {t("admin.unsuspend")}
            </Button>
          ) : (
            <Button
              variant="danger"
              onClick={() => {
                change.reset();
                setConfirming(true);
              }}
            >
              {t("admin.suspendEllipsis")}
            </Button>
          )}
          <Text tone="tertiary" size="sm">
            {suspended ? t("admin.unsuspendHint") : t("admin.suspendHint")}
          </Text>
        </div>
      )}
      {change.isError ? (
        <Banner
          tone="danger"
          role="alert"
          title={refused ? t("admin.isAdmin.title") : t("admin.failed.title")}
        >
          {refused ? t("admin.isAdmin.lead") : t("admin.failed.lead")}
        </Banner>
      ) : null}
      {change.isSuccess ? (
        <Banner
          icon="check"
          role="status"
          title={
            suspended ? t("admin.done.suspended", { name }) : t("admin.done.unsuspended", { name })
          }
        />
      ) : null}
    </div>
  );
}

import { useNavigate, useParams } from "@tanstack/react-router";
import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { useShortcuts } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  ButtonLink,
  Heading,
  HomeLayout,
  KbdHint,
  Line,
  PageHero,
  Text,
} from "~/lib/ui";
import { ScoutOffer } from "~/modules/assistant";
import { WhatNeedsYou } from "~/modules/attention";
import { ConnectedMailboxes, useConnections } from "~/modules/connectors";
import { useInboxes } from "~/modules/conversations";
import { MailboxOffer, useHeldGrants } from "~/modules/identity";
import { useWorkspaces } from "~/modules/workspaces";

/** Shortcuts worth knowing on day one, from the registry. */
const STARTER_SHORTCUTS = ["command.open", "workspace.switch", "nav.settings", "sheet.open"];

/** Settings → Mailboxes, as an in-app link (a real href, followed without a reload). */
function useMailboxesLink(workspaceId: string) {
  const navigate = useNavigate();
  const href = `/w/${workspaceId}/settings/mailboxes`;
  return {
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      void navigate({ to: href });
    },
  };
}

/**
 * Home's mail section: the mailbox held since sign-up (identity), else the
 * mailboxes connected here (connectors), else "Connect your mail". Each
 * leads to Settings → Mailboxes.
 */
function MailSection({ workspaceId }: { workspaceId: string }) {
  const { t } = useTranslation();
  const held = useHeldGrants();
  const connections = useConnections(workspaceId);
  const workspaces = useWorkspaces();
  const mailboxes = useMailboxesLink(workspaceId);
  const [grant] = held.data ?? [];
  const name = workspaces.data?.find((workspace) => workspace.id === workspaceId)?.name ?? "";

  if (grant !== undefined) {
    return (
      <MailboxOffer
        grant={grant}
        workspaceId={workspaceId}
        workspaceName={name}
        other={mailboxes}
      />
    );
  }
  if (connections.data !== undefined && connections.data.length > 0) {
    return (
      <ConnectedMailboxes
        workspaceId={workspaceId}
        connections={connections.data}
        manage={mailboxes}
      />
    );
  }
  return (
    <section className="flex flex-col items-start gap-3" aria-labelledby="connect-mail">
      <Heading level={2} size="title" id="connect-mail">
        {t("home.connectTitle")}
      </Heading>
      <Text tone="secondary">{t("home.connectLead")}</Text>
      <ButtonLink variant="primary" size="lg" icon="mail" {...mailboxes}>
        {t("home.connect")}
      </ButtonLink>
    </section>
  );
}

/**
 * A workspace's Home: what needs the user once they can see an inbox
 * (attention), else the first-run Home.
 */
export function Home() {
  const { workspaceId } = useParams({ strict: false });
  const inboxes = useInboxes(workspaceId ?? "");
  if (inboxes.isPending) {
    // A moment, so neither Home flashes before the other.
    return null;
  }
  if (inboxes.data !== undefined && inboxes.data.length > 0) {
    return (
      <WhatNeedsYou
        workspaceId={workspaceId ?? ""}
        offer={<ScoutOffer workspaceId={workspaceId ?? ""} />}
      />
    );
  }
  return <EmptyHome />;
}

/**
 * A workspace's Home before anything can need the user: says so, the mail
 * section, and the keyboard shortcuts that already work.
 */
function EmptyHome() {
  const { t, i18n } = useTranslation();
  // Shortcut labels are namespaced keys only known at runtime.
  const translate = t as unknown as (key: string) => string;
  const { registry } = useShortcuts();
  const navigate = useNavigate();
  const { workspaceId } = useParams({ strict: false });
  const today = new Intl.DateTimeFormat(i18n.language, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(new Date());

  return (
    <HomeLayout
      asideLabel={t("home.context")}
      aside={
        <>
          <AsideSection title={t("home.start")}>
            <Line
              trailing={
                <Button
                  variant="link"
                  onClick={() => {
                    void navigate({ to: `/w/${workspaceId ?? ""}/settings/members` });
                  }}
                >
                  {t("home.inviteAction")}
                </Button>
              }
            >
              {t("home.invite")}
            </Line>
          </AsideSection>
          <AsideSection title={t("home.keyboard")}>
            {STARTER_SHORTCUTS.map((id) => {
              const definition = registry.get(id);
              return definition === undefined ? null : (
                <Line key={id} trailing={<KbdHint shortcut={id} announce />}>
                  {translate(definition.label)}
                </Line>
              );
            })}
          </AsideSection>
        </>
      }
    >
      <PageHero
        visor="happy"
        dateLine={today}
        title={t("home.title")}
        subline={t("home.subline")}
      />
      <MailSection workspaceId={workspaceId ?? ""} />
    </HomeLayout>
  );
}

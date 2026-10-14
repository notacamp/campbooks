import { useNavigate, useParams } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { useShortcuts } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  Heading,
  HomeLayout,
  KbdHint,
  Line,
  PageHero,
  Tag,
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

/**
 * Home's mail section: the mailbox held since sign-up (identity), else the
 * mailboxes connected here (connectors), else "Connect your mail", honestly
 * not ready yet.
 */
function MailSection({ workspaceId }: { workspaceId: string }) {
  const { t } = useTranslation();
  const held = useHeldGrants();
  const connections = useConnections(workspaceId);
  const workspaces = useWorkspaces();
  const [grant] = held.data ?? [];
  const name = workspaces.data?.find((workspace) => workspace.id === workspaceId)?.name ?? "";

  if (grant !== undefined) {
    return <MailboxOffer grant={grant} workspaceId={workspaceId} workspaceName={name} />;
  }
  if (connections.data !== undefined && connections.data.length > 0) {
    return <ConnectedMailboxes connections={connections.data} />;
  }
  return (
    <section className="flex flex-col items-start gap-3" aria-labelledby="connect-mail">
      <div className="flex flex-wrap items-center gap-2">
        <Heading level={2} size="title" id="connect-mail">
          {t("home.connectTitle")}
        </Heading>
        <Tag>{t("home.soon")}</Tag>
      </div>
      <Text tone="secondary">{t("home.connectLead")}</Text>
      <Button variant="primary" size="lg" icon="mail" disabled>
        {t("home.connect")}
      </Button>
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

import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { ButtonLink, Heading, Line, StatusText } from "~/lib/ui";

import { useMembers } from "~/modules/workspaces";

import type { Connection } from "../../api/queries";
import { healthTone, knownMailProvider } from "../../model";

/** An in-app link: a real href, followed without a reload. */
export interface InAppLink {
  href: string;
  onClick: (event: MouseEvent<HTMLElement>) => void;
}

export interface ConnectedMailboxesProps {
  workspaceId: string;
  connections: readonly Connection[];
  /** Settings → Mailboxes. */
  manage: InAppLink;
}

/** Home's mail section once mailboxes are connected: each one, how it's doing, and where to manage them. */
export function ConnectedMailboxes({ workspaceId, connections, manage }: ConnectedMailboxesProps) {
  const { t } = useTranslation("connectors");
  const members = useMembers(workspaceId);
  const health = (connection: Connection) => {
    const provider = knownMailProvider(connection.provider)
      ? t(`provider.${connection.provider}`)
      : connection.provider;
    switch (connection.health) {
      case "ok": {
        return t("health.ok");
      }
      case "needs_reauth": {
        if (connection.granted_by_you) {
          return t("health.needsYou");
        }
        // Only its grantor can reconnect it: say who.
        const grantor = members.data?.find((member) => member.user_id === connection.granted_by);
        const name = grantor?.display_name ?? grantor?.email ?? undefined;
        return name === undefined ? t("health.needsAnother") : t("health.needsThem", { name });
      }
      case "throttled": {
        return t("health.throttled", { provider });
      }
      case "failing": {
        return t("health.failing");
      }
    }
  };
  return (
    <section className="flex flex-col items-start gap-3" aria-labelledby="connected-mail">
      <Heading level={2} size="title" id="connected-mail">
        {t("mail.title")}
      </Heading>
      <div className="flex w-full flex-col">
        {connections.map((connection) => (
          <Line
            key={connection.id}
            trailing={
              <StatusText tone={healthTone(connection.health)}>{health(connection)}</StatusText>
            }
          >
            {connection.address}
          </Line>
        ))}
      </div>
      <ButtonLink variant="link" {...manage}>
        {t("mail.manage")}
      </ButtonLink>
    </section>
  );
}

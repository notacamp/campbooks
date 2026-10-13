import type { MouseEvent } from "react";
import { useTranslation } from "react-i18next";

import { ButtonLink, Heading, Line, StatusText } from "~/lib/ui";

import type { Connection } from "../../api/queries";
import { healthTone, knownMailProvider } from "../../model";

/** An in-app link: a real href, followed without a reload. */
export interface InAppLink {
  href: string;
  onClick: (event: MouseEvent<HTMLElement>) => void;
}

export interface ConnectedMailboxesProps {
  connections: readonly Connection[];
  /** Settings → Mailboxes. */
  manage: InAppLink;
}

/** Home's mail section once mailboxes are connected: each one, how it's doing, and where to manage them. */
export function ConnectedMailboxes({ connections, manage }: ConnectedMailboxesProps) {
  const { t } = useTranslation("connectors");
  const health = (connection: Connection) => {
    const provider = knownMailProvider(connection.provider)
      ? t(`provider.${connection.provider}`)
      : connection.provider;
    switch (connection.health) {
      case "ok": {
        return t("health.ok");
      }
      case "needs_reauth": {
        return t("health.needsYou");
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

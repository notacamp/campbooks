import { useTranslation } from "react-i18next";

import { Heading, Line, Text } from "~/lib/ui";

import type { Connection } from "../../api/queries";

export interface ConnectedMailboxesProps {
  connections: readonly Connection[];
}

/** Home's mail section once mailboxes are connected: each one, and how it's doing. */
export function ConnectedMailboxes({ connections }: ConnectedMailboxesProps) {
  const { t } = useTranslation("connectors");
  return (
    <section className="flex flex-col gap-3" aria-labelledby="connected-mail">
      <Heading level={2} size="title" id="connected-mail">
        {t("mail.title")}
      </Heading>
      <div className="flex flex-col">
        {connections.map((connection) => (
          <Line
            key={connection.id}
            trailing={
              <Text as="span" size="sm" tone={connection.health === "ok" ? "tertiary" : "danger"}>
                {t(`mail.health.${connection.health}`)}
              </Text>
            }
          >
            {connection.address}
          </Line>
        ))}
      </div>
    </section>
  );
}

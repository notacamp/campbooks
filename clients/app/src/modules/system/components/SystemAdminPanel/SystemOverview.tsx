import type { ReactNode } from "react";
import type { TFunction } from "i18next";
import { useTranslation } from "react-i18next";

import { Fact, FactList, type FactStatus, Heading, Text } from "~/lib/ui";

import type { AdminOverview, AiRoleStatus } from "../../api/admin";

/** Days, hours and minutes, the two largest that apply ("3 days, 4 hours"). */
export function formatUptime(seconds: number, locale: string): string | undefined {
  const units = [
    ["day", Math.floor(seconds / 86_400)],
    ["hour", Math.floor((seconds % 86_400) / 3600)],
    ["minute", Math.floor((seconds % 3600) / 60)],
  ] as const;
  const parts = units
    .filter(([, value]) => value > 0)
    .slice(0, 2)
    .map(([unit, value]) =>
      new Intl.NumberFormat(locale, { style: "unit", unit, unitDisplay: "long" }).format(value),
    );
  return parts.length === 0
    ? undefined
    : new Intl.ListFormat(locale, { style: "long", type: "unit" }).format(parts);
}

function Section({ title, lead, children }: { title: string; lead?: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <Heading level={2} size="section">
        {title}
      </Heading>
      {lead === undefined ? null : <Text tone="secondary">{lead}</Text>}
      <FactList>{children}</FactList>
    </section>
  );
}

/** The system overview, as the admin System panel shows it. */
export function SystemOverview({ overview }: { overview: AdminOverview }) {
  const { t, i18n } = useTranslation("system");
  const locale = i18n.language;
  const dateTime = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  const number = new Intl.NumberFormat(locale);
  const { configuration: config, database, services } = overview;
  const onOff = (on: boolean) => (on ? "on" : "off");
  const providers = (names: string[]) =>
    names.length === 0
      ? t("admin.values.none")
      : new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(
          names.map((name) =>
            name === "google" || name === "microsoft" || name === "zoho"
              ? t(`admin.values.provider.${name}`)
              : name,
          ),
        );

  return (
    <div className="flex flex-col gap-8">
      <Section title={t("admin.server.title")}>
        <Fact label={t("admin.server.version")}>{overview.version}</Fact>
        <Fact label={t("admin.server.commit")}>
          {overview.commit ?? t("admin.server.commitUnknown")}
        </Fact>
        <Fact label={t("admin.server.started")}>
          {dateTime.format(new Date(overview.started_at))}
        </Fact>
        <Fact label={t("admin.server.uptime")}>
          {formatUptime(overview.uptime_seconds, locale) ?? t("admin.duration.lessThanAMinute")}
        </Fact>
      </Section>

      <Section title={t("admin.database.title")}>
        <Fact
          label={t("admin.database.status")}
          status={database.reachable ? "ok" : "problem"}
          detail={
            database.latency_ms === undefined || database.latency_ms === null
              ? undefined
              : t("admin.database.latency", { ms: number.format(database.latency_ms) })
          }
        >
          {database.reachable ? t("admin.database.reachable") : t("admin.database.unreachable")}
        </Fact>
        <Fact
          label={t("admin.database.migrations")}
          detail={
            database.latest_migration === undefined || database.latest_migration === null
              ? undefined
              : t("admin.database.latest", {
                  version: String(database.latest_migration),
                  description: database.latest_migration_description ?? "",
                })
          }
        >
          {database.migrations_applied === undefined || database.migrations_applied === null
            ? t("admin.database.unknown")
            : number.format(database.migrations_applied)}
        </Fact>
      </Section>

      <Section title={t("admin.configuration.title")}>
        <Fact label={t("admin.configuration.environment")}>
          {t(`admin.values.environment.${config.environment}`)}
        </Fact>
        <Fact label={t("admin.configuration.publicOrigin")}>
          {config.public_origin ?? t("admin.configuration.notSet")}
        </Fact>
        <Fact label={t("admin.configuration.signupPolicy")}>
          {t(`admin.values.signupPolicy.${config.signup_policy}`)}
        </Fact>
        <Fact label={t("admin.configuration.blobStore")}>
          {t(`admin.values.blobStore.${config.blob_store}`)}
        </Fact>
        <Fact label={t("admin.configuration.databaseLogins")}>
          {t(`admin.values.databaseLogins.${config.database_logins}`)}
        </Fact>
        <Fact label={t("admin.configuration.migrations")}>
          {t(`admin.values.migrations.${config.migrations}`)}
        </Fact>
        <Fact label={t("admin.configuration.webClient")}>
          {t(`admin.values.webClient.${onOff(config.serves_web_client)}`)}
        </Fact>
        <Fact label={t("admin.configuration.telemetry")}>
          {t(`admin.values.telemetry.${onOff(config.exports_telemetry)}`)}
        </Fact>
        <Fact label={t("admin.configuration.metricsEndpoint")}>
          {t(`admin.values.metricsEndpoint.${onOff(config.metrics_endpoint)}`)}
        </Fact>
      </Section>

      <Section title={t("admin.services.title")}>
        <Fact
          label={t("admin.services.identityProvider")}
          status={services.identity_provider === "none" ? "problem" : "ok"}
        >
          {t(`admin.values.identityProvider.${services.identity_provider}`)}
        </Fact>
        <Fact label={t("admin.services.signInProviders")}>
          {providers(services.sign_in_providers)}
        </Fact>
        <Fact label={t("admin.services.mailProviders")}>{providers(services.mail_providers)}</Fact>
      </Section>

      <Section title={t("admin.ai.title")} lead={t("admin.ai.lead")}>
        {overview.ai.map((role) => (
          <AiRoleFact key={role.role} role={role} />
        ))}
      </Section>
    </div>
  );
}

function AiRoleFact({ role }: { role: AiRoleStatus }) {
  const { t } = useTranslation("system");
  const label = t(`admin.ai.role.${role.role}`);
  if (!role.configured) {
    return (
      <Fact label={label} status="off">
        {t("admin.ai.notConfigured")}
      </Fact>
    );
  }
  const reachable = role.reachable === true;
  // A role off this instance's network that has not opted in gets no mail:
  // nothing calls it but the reachability probe.
  const refused = role.content_allowed === false;
  const where =
    role.location === "external"
      ? refused
        ? t("admin.ai.where.externalRefused")
        : t("admin.ai.where.externalAllowed")
      : role.location === "instance"
        ? t("admin.ai.where.instance")
        : undefined;
  const detail = t("admin.ai.detail", {
    model: role.model ?? "",
    endpoint: role.endpoint ?? "",
    protocol:
      role.protocol === undefined || role.protocol === null
        ? ""
        : t(`admin.ai.protocol.${role.protocol}`),
  });
  const check = refused ? undefined : answersCheck(role, t);
  const status = roleStatus(refused, reachable, check?.trusted);
  const facts = [detail, where, check?.detail].filter((part) => part !== undefined);
  return (
    <Fact label={label} status={status} detail={facts.join(" · ")}>
      {refused
        ? t("admin.ai.notSentMail")
        : reachable
          ? (check?.headline ?? t("admin.ai.answering"))
          : t("admin.ai.notAnswering")}
    </Fact>
  );
}

/**
 * A role's dot: off when it gets no mail or its answers are still being
 * checked (not used yet), a problem when it does not answer or failed its
 * answers check, ok otherwise.
 */
function roleStatus(
  refused: boolean,
  reachable: boolean,
  trusted: boolean | undefined | null,
): FactStatus {
  if (refused || trusted === null) {
    return "off";
  }
  if (!reachable || trusted === false) {
    return "problem";
  }
  return "ok";
}

/**
 * The decide role's answers check in words: whether the model got obvious
 * fictional emails right, and so whether its answers are used.
 */
function answersCheck(
  role: AiRoleStatus,
  t: TFunction<"system">,
): { headline: string; detail: string; trusted: boolean | null } | undefined {
  const check = role.answers_check;
  if (check === undefined || check === null) {
    return undefined;
  }
  const counts = { right: check.right ?? 0, total: check.total ?? 0 };
  switch (check.state) {
    case "pending":
      return {
        headline: t("admin.ai.check.pending"),
        detail: t("admin.ai.check.pendingDetail"),
        trusted: null,
      };
    case "passed":
      return {
        headline: t("admin.ai.answering"),
        detail: t("admin.ai.check.passed", counts),
        trusted: true,
      };
    case "failed":
      return {
        headline: t("admin.ai.check.failed"),
        detail: t("admin.ai.check.failedDetail", counts),
        trusted: false,
      };
    case "unavailable":
      return {
        headline: t("admin.ai.check.unavailable"),
        detail: t("admin.ai.check.unavailableDetail"),
        trusted: false,
      };
  }
}

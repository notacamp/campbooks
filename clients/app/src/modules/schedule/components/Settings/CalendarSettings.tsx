import { useState } from "react";
import { useTranslation } from "react-i18next";

import { readTabValue, writeTabValue } from "~/lib/storage/tab";
import {
  Banner,
  Button,
  CalendarColour,
  CalendarDot,
  Chip,
  ChipRow,
  KindIcon,
  List,
  ListRow,
  Select,
  SettingsEmpty,
  SettingsPage,
  SettingsSection,
  Switch,
  Text,
  TimePicker,
} from "~/lib/ui";
import {
  useConnections,
  type ConsentRefusal,
  type MailboxConsent,
  type MailboxResult,
} from "~/modules/connectors";

import { useCalendars, useScheduleSettings, useSettingsMutations } from "../../api/queries";
import {
  SCOUT_KINDS,
  type CalendarView,
  type EventKind,
  type SettingsView,
  type WorkingHours,
} from "../../api/types";

const WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;
type Weekday = (typeof WEEKDAYS)[number];

const DEFAULT_RANGE = { start: "09:00", end: "18:00" } as const;

/** The days with working hours, and the one range shown for them. */
function workingOf(hours: WorkingHours): { days: Weekday[]; start: string; end: string } {
  const days = WEEKDAYS.filter((day) => (hours[day] ?? []).length > 0);
  const range = days.map((day) => hours[day]?.[0]).find((each) => each !== undefined);
  return { days, start: range?.start ?? DEFAULT_RANGE.start, end: range?.end ?? DEFAULT_RANGE.end };
}

function hoursFor(days: readonly Weekday[], start: string, end: string): WorkingHours {
  return Object.fromEntries(days.map((day) => [day, [{ start, end }]]));
}

/** The IANA zones the browser knows, with the member's own among them. */
function zoneOptions(current: string): { value: string; label: string }[] {
  const known =
    typeof Intl.supportedValuesOf === "function" ? Intl.supportedValuesOf("timeZone") : [];
  const zones = known.includes(current) ? known : [current, ...known];
  return zones.map((zone) => ({ value: zone, label: zone.replaceAll("_", " ") }));
}

const PROVIDER_NAMES: Record<string, string> = {
  google: "Google",
  zoho: "Zoho",
  microsoft: "Microsoft",
};

export interface CalendarSettingsProps {
  workspaceId: string;
  /** The front door: starts the provider's consent for calendar access. */
  consent: MailboxConsent;
  /** How the provider's screens came back here (`?connected=`, `?link_error=`), once. */
  result?: MailboxResult | undefined;
}

/**
 * Settings → Calendar: the calendars (shown or not, in which colour), what
 * Scout adds from mail and where else each kind goes, the time zone and the
 * working hours, and, for a mailbox without it, Add calendar access.
 */
export function CalendarSettings({ workspaceId, consent, result }: CalendarSettingsProps) {
  const { t } = useTranslation("schedule");
  const settings = useScheduleSettings(workspaceId);
  const calendars = useCalendars(workspaceId);
  const connections = useConnections(workspaceId);
  const { settings: save, calendar: saveCalendar } = useSettingsMutations(workspaceId);

  return (
    <SettingsPage label={t("settings.label")} title={t("settings.title")} lead={t("settings.lead")}>
      <CalendarsSection
        calendars={calendars.data}
        connections={connections.data}
        consent={consent}
        result={result}
        onShow={(calendar, syncing) => {
          saveCalendar.mutate({ calendarId: calendar.id, patch: { syncing } });
        }}
        onColour={(calendar, color) => {
          saveCalendar.mutate({ calendarId: calendar.id, patch: { color } });
        }}
      />
      {settings.data === undefined ? null : (
        <>
          <KindsSection
            settings={settings.data}
            calendars={calendars.data ?? []}
            onChange={(patch) => {
              save.mutate(patch);
            }}
          />
          <TimeSection
            settings={settings.data}
            onChange={(patch) => {
              save.mutate(patch);
            }}
          />
        </>
      )}
    </SettingsPage>
  );
}

function CalendarsSection({
  calendars,
  connections,
  consent,
  result,
  onShow,
  onColour,
}: {
  calendars: readonly CalendarView[] | undefined;
  connections: Connections | undefined;
  consent: MailboxConsent;
  result: MailboxResult | undefined;
  onShow: (calendar: CalendarView, syncing: boolean) => void;
  onColour: (calendar: CalendarView, color: string) => void;
}) {
  const { t } = useTranslation("schedule");
  // What the server offers each connection (Google always; Zoho while its adapter is on).
  const withoutAccess = (connections ?? []).filter(
    (connection) =>
      connection.offers.includes("calendar") && !connection.capabilities.includes("calendar"),
  );
  // The mailbox this return was for: the one asked about, else the only one that could be.
  const [attempt] = useState(() => readTabValue("calendarAccess.connection"));
  // Only a mailbox still without access can have been refused it.
  const asked = withoutAccess.find((connection) => connection.id === attempt);
  const only = withoutAccess.length === 1 ? withoutAccess[0] : undefined;
  return (
    <SettingsSection
      title={t("settings.calendars.title")}
      description={t("settings.calendars.description")}
    >
      <Outcome result={result} connections={connections} mailbox={asked ?? only} />
      {calendars === undefined ? null : (
        <List label={t("settings.calendars.title")}>
          {calendars.map((calendar) => {
            const own = calendar.source === "campbooks";
            const facts = [
              own ? t("settings.calendars.campbooks") : undefined,
              calendar.primary ? t("settings.calendars.primary") : undefined,
              calendar.source === "provider" && !calendar.writable
                ? t("settings.calendars.readOnly")
                : undefined,
            ].filter((each) => each !== undefined);
            return (
              <ListRow
                key={calendar.id}
                leading={
                  own ? (
                    <CalendarDot color={calendar.color} />
                  ) : (
                    <CalendarColour
                      label={t("settings.calendars.colour", { name: calendar.name })}
                      color={calendar.color}
                      onChange={(color) => {
                        onColour(calendar, color);
                      }}
                    />
                  )
                }
                title={calendar.name}
                meta={facts.length === 0 ? undefined : facts.join(" · ")}
                trailing={
                  own ? (
                    <Text as="span" tone="tertiary" size="sm">
                      {t("settings.calendars.alwaysOn")}
                    </Text>
                  ) : (
                    <Switch
                      label={t("settings.calendars.show", { name: calendar.name })}
                      checked={calendar.syncing}
                      onChange={(syncing) => {
                        onShow(calendar, syncing);
                      }}
                    />
                  )
                }
              />
            );
          })}
        </List>
      )}
      {withoutAccess.length === 0 ? null : (
        <List label={t("settings.calendars.access")}>
          {withoutAccess.map((connection) => (
            <ListRow
              key={connection.id}
              title={connection.address}
              meta={t("settings.calendars.accessHint")}
              trailing={
                connection.granted_by_you ? (
                  <Button
                    disabled={consent.starting}
                    onClick={() => {
                      writeTabValue("calendarAccess.connection", connection.id);
                      consent.start({
                        purpose: "reconnect",
                        connection: connection.id,
                        capabilities: ["calendar"],
                      });
                    }}
                  >
                    {t("settings.calendars.access")}
                  </Button>
                ) : null
              }
            />
          ))}
        </List>
      )}
      {consent.refusal === undefined ? null : (
        <Refusal consent={consent} refusal={consent.refusal} />
      )}
    </SettingsSection>
  );
}

type Connections = NonNullable<ReturnType<typeof useConnections>["data"]>;

/** What the provider's screens came back with, said once above the calendars. */
function Outcome({
  result,
  connections,
  mailbox,
}: {
  result: MailboxResult | undefined;
  connections: Connections | undefined;
  mailbox: Connections[number] | undefined;
}) {
  const { t } = useTranslation("schedule");
  if (result === undefined) {
    return null;
  }
  if (result.kind === "connected") {
    const address = connections?.find((connection) => connection.id === result.connection)?.address;
    return (
      <Banner
        role="status"
        icon="check"
        title={
          address === undefined
            ? t("settings.calendars.outcome.added")
            : t("settings.calendars.outcome.addedFor", { address })
        }
      />
    );
  }
  if (result.error === "account_mismatch") {
    const provider = mailbox === undefined ? undefined : PROVIDER_NAMES[mailbox.provider];
    return (
      <Banner
        tone="danger"
        role="alert"
        title={
          mailbox === undefined || provider === undefined
            ? t("settings.calendars.outcome.mismatchAnother")
            : t("settings.calendars.outcome.mismatch", { provider, address: mailbox.address })
        }
      />
    );
  }
  return (
    <Banner
      tone="danger"
      role="alert"
      title={
        result.error === "cancelled_at_provider"
          ? t("settings.calendars.outcome.cancelled")
          : t("settings.calendars.outcome.failed")
      }
    />
  );
}

/** Why adding calendar access was refused, in words that say what to do. */
function Refusal({ consent, refusal }: { consent: MailboxConsent; refusal: ConsentRefusal }) {
  const { t } = useTranslation("schedule");
  switch (refusal) {
    case "step_up":
      return (
        <Banner
          role="alert"
          icon="key"
          title={t("settings.calendars.stepUp.title")}
          action={
            <Button variant="secondary" onClick={consent.signInAgain}>
              {t("settings.calendars.stepUp.action")}
            </Button>
          }
        >
          {t("settings.calendars.stepUp.lead")}
        </Banner>
      );
    case "unsupported":
      return <SettingsEmpty title={t("settings.calendars.unsupported")} />;
    case "rate_limited":
      return <SettingsEmpty title={t("settings.calendars.rateLimited")} />;
    case "failed":
      return <SettingsEmpty title={t("settings.calendars.refused")} />;
  }
}

function KindsSection({
  settings,
  calendars,
  onChange,
}: {
  settings: SettingsView;
  calendars: readonly CalendarView[];
  onChange: (patch: { scout_kinds?: EventKind[]; also_add_to?: Record<string, string> }) => void;
}) {
  const { t } = useTranslation("schedule");
  const targets = calendars.filter((each) => each.source === "provider" && each.writable);
  const options = [
    { value: "", label: t("settings.kinds.nowhere") },
    ...targets.map((each) => ({ value: each.id, label: each.name })),
  ];
  return (
    <SettingsSection
      title={t("settings.kinds.title")}
      description={t("settings.kinds.description")}
    >
      <List label={t("settings.kinds.title")}>
        {SCOUT_KINDS.map((kind) => {
          const on = settings.scout_kinds.includes(kind);
          const name = t(`settings.kindNames.${kind}`);
          return (
            <ListRow
              key={kind}
              leading={<KindIcon kind={kind} />}
              title={name}
              meta={t(`settings.kinds.${kind}`)}
              trailing={
                <>
                  {targets.length === 0 ? null : (
                    <Select
                      hideLabel
                      label={t("settings.kinds.alsoAdd")}
                      value={settings.also_add_to[kind] ?? ""}
                      options={options}
                      onChange={(value) => {
                        const others = Object.entries(settings.also_add_to).filter(
                          ([each]) => each !== kind,
                        );
                        onChange({
                          also_add_to: Object.fromEntries(
                            value === "" ? others : [...others, [kind, value]],
                          ),
                        });
                      }}
                    />
                  )}
                  <Switch
                    label={t("settings.kinds.add", { kind: name })}
                    checked={on}
                    onChange={(checked) => {
                      onChange({
                        scout_kinds: checked
                          ? [...settings.scout_kinds, kind]
                          : settings.scout_kinds.filter((each) => each !== kind),
                      });
                    }}
                  />
                </>
              }
            />
          );
        })}
      </List>
    </SettingsSection>
  );
}

function TimeSection({
  settings,
  onChange,
}: {
  settings: SettingsView;
  onChange: (patch: { time_zone?: string; working_hours?: WorkingHours }) => void;
}) {
  const { t } = useTranslation("schedule");
  const working = workingOf(settings.working_hours);
  return (
    <SettingsSection title={t("settings.time.title")} description={t("settings.time.description")}>
      <Select
        label={t("settings.time.zone")}
        hint={t("settings.time.zoneHint")}
        value={settings.effective_time_zone}
        options={zoneOptions(settings.effective_time_zone)}
        onChange={(zone) => {
          onChange({ time_zone: zone });
        }}
      />
      <div className="flex flex-col gap-2">
        <Text as="span" weight="medium">
          {t("settings.time.days")}
        </Text>
        <ChipRow label={t("settings.time.days")}>
          {WEEKDAYS.map((day) => (
            <Chip
              key={day}
              pressed={working.days.includes(day)}
              onClick={() => {
                const days = working.days.includes(day)
                  ? working.days.filter((each) => each !== day)
                  : [...working.days, day];
                onChange({ working_hours: hoursFor(days, working.start, working.end) });
              }}
            >
              {t(`settings.time.weekdays.${day}`)}
            </Chip>
          ))}
        </ChipRow>
      </div>
      {/* Remounted when the server's hours change, so another device's edit shows. */}
      <HoursFields
        key={`${working.start}-${working.end}`}
        start={working.start}
        end={working.end}
        onCommit={(start, end) => {
          onChange({ working_hours: hoursFor(working.days, start, end) });
        }}
      />
    </SettingsSection>
  );
}

function HoursFields({
  start: savedStart,
  end: savedEnd,
  onCommit,
}: {
  start: string;
  end: string;
  onCommit: (start: string, end: string) => void;
}) {
  const { t } = useTranslation("schedule");
  const [start, setStart] = useState(savedStart);
  const [end, setEnd] = useState(savedEnd);
  // A picked or typed time is whole: it is saved at once.
  const commit = (from: string, to: string) => {
    if (from !== "" && to !== "" && (from !== savedStart || to !== savedEnd)) {
      onCommit(from, to);
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <Text as="span" weight="medium">
        {t("settings.time.hours")}
      </Text>
      <div className="flex flex-col gap-3 sm:flex-row">
        <div className="flex-1">
          <TimePicker
            label={t("settings.time.from")}
            value={start === "" ? undefined : start}
            onChange={(next) => {
              setStart(next);
              commit(next, end);
            }}
          />
        </div>
        <div className="flex-1">
          <TimePicker
            label={t("settings.time.to")}
            value={end === "" ? undefined : end}
            min={start === "" ? undefined : start}
            onChange={(next) => {
              setEnd(next);
              commit(start, next);
            }}
          />
        </div>
      </div>
    </div>
  );
}

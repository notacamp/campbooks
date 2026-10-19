import { Button, Text } from "~/lib/ui";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

/** One conversation Scout waits on, as the list shows it. */
export interface WaitingRow {
  /** The attention item. */
  id: string;
  /** Who Scout waits on: "Ana Costa". */
  name: string;
  /** The conversation's subject (a few words of it are shown). */
  subject: string;
  /** When Scout checks again, in words ("Checks Mon"). */
  checks: string;
  /** The full date it checks, for the tooltip and machines. */
  checksOn: { dateTime: string; label: string } | undefined;
  /** How many nudges went since. */
  nudges: number;
}

export interface WaitingOnOthersProps {
  rows: readonly WaitingRow[];
  t: Translate;
  onNudge: (row: WaitingRow) => void;
  onNotWaiting: (row: WaitingRow) => void;
}

/** Most words of a subject shown in a row. */
const SUBJECT_WORDS = 6;

/** "Revised floor plan for the kitchen and…": the first few words. */
function fewWords(subject: string): string {
  const words = subject.trim().split(/\s+/u);
  return words.length <= SUBJECT_WORDS
    ? subject.trim()
    : `${words.slice(0, SUBJECT_WORDS).join(" ")}…`;
}

/**
 * "Waiting on others" (campbooks-cloud#293): who Scout waits on for the
 * member, soonest check first, each with Nudge now and Not waiting, both in
 * the row's one button style.
 */
export function WaitingOnOthers({ rows, t, onNudge, onNotWaiting }: WaitingOnOthersProps) {
  return (
    <ul className="flex flex-col gap-3">
      {rows.map((row) => (
        <li key={row.id} aria-label={t("waiting.label", { name: row.name })}>
          <div className="flex min-w-0 items-baseline gap-2">
            <Text as="span" size="sm" weight="semibold" truncate className="min-w-0 flex-1">
              {row.name}
            </Text>
            {row.checksOn === undefined ? (
              <Text as="span" size="xs" tone="tertiary" numeric>
                {row.checks}
              </Text>
            ) : (
              <Text as="time" size="xs" tone="tertiary" numeric dateTime={row.checksOn.dateTime}>
                <span title={row.checksOn.label}>{row.checks}</span>
              </Text>
            )}
          </div>
          <Text size="sm" tone="secondary" truncate>
            {row.nudges > 0
              ? `${fewWords(row.subject)} · ${t("nudge.nudged", { count: row.nudges })}`
              : fewWords(row.subject)}
          </Text>
          <div className="flex flex-wrap gap-x-3">
            <Button
              variant="link"
              ai
              onClick={() => {
                onNudge(row);
              }}
            >
              {t("waiting.nudgeNow")}
            </Button>
            <Button
              variant="link"
              onClick={() => {
                onNotWaiting(row);
              }}
            >
              {t("waiting.notWaiting")}
            </Button>
          </div>
        </li>
      ))}
    </ul>
  );
}

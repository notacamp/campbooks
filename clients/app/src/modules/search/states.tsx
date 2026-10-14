import type { ReactNode } from "react";

import { errorStatus, problemCode } from "~/lib/api";
import {
  Banner,
  Button,
  EmptyState,
  Heading,
  Icon,
  Line,
  LineMeta,
  Spacer,
  Text,
  VisuallyHidden,
} from "~/lib/ui";

import type { SearchWarning } from "./api/queries";
import { warningText, type Translate } from "./model";

/** What to show for a failed search. */
export type SearchProblemKind = "too_long" | "cursor" | "busy" | "other";

function problemKind(error: unknown): SearchProblemKind {
  const code = problemCode(error);
  if (code === "query_too_long") {
    return "too_long";
  }
  if (code === "cursor_mismatch") {
    return "cursor";
  }
  return errorStatus(error) === 429 ? "busy" : "other";
}

/** An error, said plainly, with the one thing to do about it. */
export function SearchProblem({
  t,
  error,
  onRetry,
  compact = false,
}: {
  t: Translate;
  error: unknown;
  /** Searches again (from the first page, for an out-of-date cursor). */
  onRetry: () => void;
  compact?: boolean;
}) {
  const kind = problemKind(error);
  // A too-long query is fixed by editing it, not by trying again.
  const action =
    kind === "too_long" ? undefined : (
      <Button onClick={onRetry}>{t(kind === "cursor" ? "error.again" : "error.retry")}</Button>
    );
  if (compact) {
    return (
      <div className="p-2">
        <Banner tone="danger" role="alert" title={t(`error.${kind}.title`)} action={action}>
          {t(`error.${kind}.body`)}
        </Banner>
      </div>
    );
  }
  return (
    <EmptyState title={t(`error.${kind}.title`)} visor="thinking" visorSize="lg" action={action}>
      {t(`error.${kind}.body`)}
    </EmptyState>
  );
}

/** "Still indexing older mail": results may be missing for now. */
export function IndexingNote({ t }: { t: Translate }) {
  return (
    <div className="px-2 pt-2">
      <Banner tone="neutral" icon="clock" role="status" title={t("indexing.title")}>
        {t("indexing.body")}
      </Banner>
    </div>
  );
}

/** Parts of the query the server did not understand, one plain line each. */
export function Warnings({ t, warnings }: { t: Translate; warnings: readonly SearchWarning[] }) {
  const lines = warnings.map((warning) => warningText(t, warning)).filter((line) => line !== "");
  if (lines.length === 0) {
    return null;
  }
  return (
    <ul className="flex flex-col gap-1 px-3 pt-2" aria-label={t("warning.label")}>
      {lines.map((line) => (
        <li key={line}>
          <Text size="sm" tone="secondary">
            {line}
          </Text>
        </li>
      ))}
    </ul>
  );
}

/** Nothing matched: say so, and point at a chip to remove when there is one. */
export function NoResults({
  t,
  query,
  hasFilters,
}: {
  t: Translate;
  query: string;
  hasFilters: boolean;
}) {
  return (
    <EmptyState title={t("empty.title", { query })} visor="unsure" visorSize="lg">
      {hasFilters ? t("empty.bodyFilters") : t("empty.body")}
    </EmptyState>
  );
}

/** The device's recent searches; each runs with one click. */
export function RecentSearches({
  t,
  items,
  onPick,
  onClear,
}: {
  t: Translate;
  items: readonly string[];
  onPick: (query: string) => void;
  onClear: () => void;
}) {
  if (items.length === 0) {
    return null;
  }
  return (
    <section className="flex flex-col gap-1 px-3 pt-3" aria-labelledby="search-recent">
      <div className="flex items-center gap-2">
        <Heading level={2} size="section" id="search-recent">
          {t("recent.title")}
        </Heading>
        <Spacer />
        <Button variant="ghost" onClick={onClear}>
          {t("recent.clear")}
        </Button>
      </div>
      <ul className="flex flex-col">
        {items.map((item) => (
          <li key={item}>
            <Button
              variant="ghost"
              icon="clock"
              grow
              onClick={() => {
                onPick(item);
              }}
            >
              {item}
            </Button>
          </li>
        ))}
      </ul>
    </section>
  );
}

const TIPS = ["from", "attachment", "unread", "phrase", "exclude", "date"] as const;

/** The operators, each with an example to type. */
export function SearchTips({ t }: { t: Translate }): ReactNode {
  return (
    <section className="flex flex-col gap-1 px-3 py-3" aria-labelledby="search-tips">
      <Heading level={2} size="section" id="search-tips">
        {t("tips.title")}
      </Heading>
      <div className="flex flex-col">
        {TIPS.map((tip) => (
          <Line
            key={tip}
            leading={<Icon name="search" />}
            trailing={
              <LineMeta>
                <code>{t(`tips.${tip}.example`)}</code>
              </LineMeta>
            }
          >
            {t(`tips.${tip}.text`)}
          </Line>
        ))}
      </div>
    </section>
  );
}

/** A live count for screen readers. */
export function ResultCount({ t, count }: { t: Translate; count: number }) {
  return (
    <p role="status">
      <VisuallyHidden>{t("count", { count })}</VisuallyHidden>
    </p>
  );
}

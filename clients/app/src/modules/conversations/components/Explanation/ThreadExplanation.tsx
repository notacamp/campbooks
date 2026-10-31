import { useEffect, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { Button, ExplanationCard, ExplanationLine, type ExplanationGroup } from "~/lib/ui";

import {
  POLL_LIMIT_MS,
  refusalOf,
  useExplain,
  useExplanation,
  type Explanation,
  type ExplanationView,
} from "../../api/explanation";
import { firstName } from "../../compose/dialog";
import { messageTime, type Translate } from "../../model";

/** How many messages the server reads in full; the card says so for a longer thread. */
export interface ThreadExplanationOptions {
  workspaceId: string;
  conversationId: string;
  /** `ConversationDetail.explain`: may this member ask? The client never probes. */
  offered: boolean;
  /** How many messages the thread has now (what Scout is reading while it writes). */
  messageCount: number;
  /** A chip was chosen: scroll to that message and open it. */
  onOpenMessage: (messageId: string) => void;
  /** Ask a follow-up: Scout's chat about this thread. */
  onFollowUp: () => void;
  /** Reply, when the member can. */
  onReply?: (() => void) | undefined;
  now?: Date | undefined;
}

export interface ThreadExplanationControl {
  /** Ask Scout to explain the thread (a new request, or "Explain again"). */
  ask: () => void;
  /** The button, the More menu and the shortcut are offered. */
  offered: boolean;
  /** A request is going out or being written: asking again waits. */
  busy: boolean;
  /** The member asked about this thread before: the card or its line is up, with its own buttons. */
  asked: boolean;
  /** The card, the one line, or nothing. */
  element: ReactNode;
}

/** The first points of each list, as groups. */
function groupsOf(
  body: Explanation,
  chipLabel: (id: string) => string | undefined,
  select: (id: string) => void,
  t: Translate,
  locale: string,
): ExplanationGroup[] {
  const chips = (ids: string[], key: string) =>
    ids.flatMap((id, at) => {
      const label = chipLabel(id);
      return label === undefined
        ? []
        : [
            {
              id: `${key}-${String(at)}`,
              label,
              onSelect: () => {
                select(id);
              },
            },
          ];
    });
  const due = (day: string | null | undefined) =>
    day === null || day === undefined
      ? undefined
      : t("explain.by", {
          date: new Intl.DateTimeFormat(locale, {
            weekday: "short",
            day: "numeric",
            month: "short",
          }).format(new Date(`${day}T12:00:00`)),
        });
  return [
    {
      id: "wants",
      label: t("explain.wants"),
      folded: true,
      items: body.wants.map((want, at) => ({
        id: `wants-${String(at)}`,
        lead: want.who === "" ? undefined : want.who,
        text: want.what,
        chips: chips(want.refs, `wants-${String(at)}`),
      })),
    },
    {
      id: "agreed",
      label: t("explain.agreed"),
      folded: true,
      items: body.agreed.map((point, at) => ({
        id: `agreed-${String(at)}`,
        text: point.text,
        chips: chips(point.refs, `agreed-${String(at)}`),
      })),
    },
    {
      id: "open",
      label: t("explain.open"),
      folded: true,
      items: body.open.map((point, at) => ({
        id: `open-${String(at)}`,
        text: point.text,
        chips: chips(point.refs, `open-${String(at)}`),
      })),
    },
    {
      id: "on-you",
      label: t("explain.onYou"),
      emphasis: true,
      items: body.on_you.map((duty, at) => ({
        id: `on-you-${String(at)}`,
        text: duty.text,
        due: due(duty.due_on),
        chips: chips(duty.refs, `on-you-${String(at)}`),
      })),
    },
  ].filter((group) => group.items.length > 0);
}

/**
 * Scout's explanation of a thread, only when the member asks (campbooks-cloud#407).
 * A thread they never asked about shows nothing and asks the server for
 * nothing but the one read that finds out whether they did; one they asked
 * about shows a single line on return, and the card opens only when they
 * choose Show or right after they ask. Nothing is explained again on its own.
 */
export function useThreadExplanation({
  workspaceId,
  conversationId,
  offered,
  messageCount,
  onOpenMessage,
  onFollowUp,
  onReply,
  now,
}: ThreadExplanationOptions): ThreadExplanationControl {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const locale = i18n.language;
  const stored = useExplanation(workspaceId, conversationId, { enabled: offered });
  const explain = useExplain(workspaceId, conversationId);
  const [open, setOpen] = useState(false);
  const view: ExplanationView | undefined = stored.data ?? undefined;
  const writing = view?.state === "writing";

  // A card that is still being written after the polling gave up says so.
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!writing) {
      return;
    }
    const timer = setTimeout(() => {
      setSlow(true);
    }, POLL_LIMIT_MS);
    return () => {
      clearTimeout(timer);
    };
  }, [writing, view?.written_at]);

  const ask = () => {
    if (!offered || explain.isPending || (writing && !slow)) {
      return;
    }
    setOpen(true);
    setSlow(false);
    explain.mutate();
  };

  const body = view?.body ?? undefined;
  const failure = view?.state === "failed" ? view.failure : undefined;
  const refused = explain.isError && view === undefined ? refusalOf(explain.error) : undefined;

  if (!offered && view === undefined) {
    return { ask, offered, busy: false, asked: false, element: null };
  }
  const busy = explain.isPending || (writing && !slow);
  const again = (label: string, primary = true) => (
    <Button variant={primary ? "primary" : "secondary"} disabled={busy} onClick={ask}>
      {label}
    </Button>
  );

  // Nothing was asked: nothing shows (the button is in the note).
  if (view === undefined) {
    if (explain.isPending) {
      return {
        ask,
        offered,
        busy: true,
        asked: false,
        element: open ? (
          <ExplanationCard
            title={t("explain.title")}
            hideLabel={t("explain.hide")}
            onHide={() => {
              setOpen(false);
            }}
            writing={{
              label: t("explain.reading", { count: messageCount }),
              stopLabel: t("explain.stop"),
              inShortLabel: t("explain.inShort"),
              onStop: () => {
                setOpen(false);
              },
            }}
          />
        ) : null,
      };
    }
    if (refused !== undefined && open) {
      return {
        ask,
        offered,
        busy: false,
        asked: false,
        element: (
          <ExplanationCard
            title={t("explain.title")}
            hideLabel={t("explain.hide")}
            onHide={() => {
              setOpen(false);
            }}
            notice={t(`explain.refused.${refused}`)}
            actions={refused === "failed" ? again(t("explain.tryAgain")) : undefined}
          />
        ),
      };
    }
    return { ask, offered, busy: false, asked: false, element: null };
  }

  const stamp = (iso: string) => messageTime(iso, locale, now);
  const written =
    view.written_at === null || view.written_at === undefined ? undefined : stamp(view.written_at);

  if (!open) {
    const newer = view.newer ?? undefined;
    const text = writing
      ? t("explain.line.writing")
      : newer !== undefined && written !== undefined
        ? t("explain.line.newer", {
            when: written,
            name: firstNameOf(newer.from),
            at: stamp(newer.sent_at),
          })
        : written === undefined
          ? t("explain.line.none")
          : t("explain.line.kept", { when: written, count: view.messages });
    return {
      ask,
      offered,
      busy,
      asked: true,
      element: (
        <ExplanationLine
          actions={
            <>
              <Button
                variant="ghost"
                onClick={() => {
                  setOpen(true);
                }}
              >
                {t("explain.show")}
              </Button>
              {view.stale || failure !== undefined ? again(t("explain.again"), false) : null}
            </>
          }
        >
          {text}
        </ExplanationLine>
      ),
    };
  }

  if (writing && body === undefined) {
    return {
      ask,
      offered,
      busy,
      asked: true,
      element: (
        <ExplanationCard
          title={t("explain.title")}
          hideLabel={t("explain.hide")}
          onHide={() => {
            setOpen(false);
          }}
          notice={slow ? t("explain.slow") : undefined}
          writing={{
            label: t("explain.reading", { count: messageCount }),
            stopLabel: t("explain.stop"),
            inShortLabel: t("explain.inShort"),
            onStop: () => {
              setOpen(false);
            },
          }}
          actions={slow ? again(t("explain.tryAgain")) : undefined}
        />
      ),
    };
  }

  const notices: string[] = [];
  // Explaining again keeps the earlier explanation in view, and says Scout
  // is at it, so the click is never met by a card that looks unchanged.
  if (writing) {
    notices.push(t("explain.reading", { count: messageCount }));
  }
  if (failure === "limit") {
    notices.push(t("explain.limit"));
  } else if (failure !== undefined) {
    notices.push(t("explain.failed"));
  }
  if (view.stale) {
    const newer = view.newer ?? undefined;
    if (newer !== undefined) {
      notices.push(
        t("explain.newer", { name: firstNameOf(newer.from), when: stamp(newer.sent_at) }),
      );
    } else if (view.locale !== null && view.locale !== undefined) {
      notices.push(
        t("explain.otherLanguage", {
          language:
            new Intl.DisplayNames([locale], { type: "language" }).of(view.locale) ?? view.locale,
        }),
      );
    }
  }
  if (body !== undefined && view.messages > view.in_full) {
    notices.push(
      t("explain.long", {
        total: view.messages,
        full: view.in_full,
        earlier: view.messages - view.in_full,
      }),
    );
  }
  const names = new Map(view.refs.map((ref) => [ref.id, ref]));
  const chipLabel = (id: string) => {
    const ref = names.get(id);
    return ref === undefined
      ? undefined
      : t("explain.chip", {
          name: firstNameOf(ref.from),
          date: new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" }).format(
            new Date(ref.sent_at),
          ),
        });
  };
  return {
    ask,
    offered,
    busy,
    asked: true,
    element: (
      <ExplanationCard
        title={t("explain.title")}
        meta={
          written === undefined
            ? undefined
            : t("explain.meta", { count: view.messages, when: written })
        }
        hideLabel={t("explain.hide")}
        onHide={() => {
          setOpen(false);
        }}
        notice={
          notices.length === 0 ? undefined : (
            <>
              {notices.map((line) => (
                <p key={line}>{line}</p>
              ))}
            </>
          )
        }
        inShort={
          body === undefined ? undefined : { label: t("explain.inShort"), text: body.in_short }
        }
        groups={body === undefined ? [] : groupsOf(body, chipLabel, onOpenMessage, t, locale)}
        more={{ label: t("explain.more"), lessLabel: t("explain.less") }}
        actions={
          <>
            {view.stale || (failure !== undefined && failure !== "limit")
              ? again(
                  failure !== undefined && !view.stale ? t("explain.tryAgain") : t("explain.again"),
                )
              : null}
            <Button variant={view.stale ? "ghost" : "secondary"} onClick={onFollowUp}>
              {t("explain.followUp")}
            </Button>
            {onReply === undefined ? null : (
              <Button variant="ghost" onClick={onReply}>
                {t("explain.reply")}
              </Button>
            )}
          </>
        }
        privacy={t("explain.privacy")}
      />
    ),
  };
}

/** The first name of whoever wrote, for a chip or a line: "Jonas". */
function firstNameOf(from: string): string {
  const first = firstName(from);
  return first === "" ? from : first;
}

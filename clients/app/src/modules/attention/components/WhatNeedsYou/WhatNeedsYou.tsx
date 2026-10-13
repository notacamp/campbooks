import { useNavigate } from "@tanstack/react-router";
import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut, useShortcuts } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  EmptyState,
  HomeLayout,
  HowDecided,
  Item,
  ItemDetail,
  KbdHint,
  Line,
  LineMeta,
  PageHero,
  Popover,
  QueueGroup,
  SkeletonList,
  useUndoToast,
} from "~/lib/ui";
import {
  dueLabel,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  useConversation,
  useConversationsByIds,
  useNeedActions,
  warningReasons,
  type ConversationSummary,
  type NeedView,
} from "~/modules/conversations";

import { useAttention, useRefreshAttention, useSnooze, type ItemView } from "../../api/queries";
import { doneTodayKind, groupByReason, laterOptions } from "../../model";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

const CONVERSATION = "conversations.conversation";

/** The keys Home teaches, from the registry. */
const KEYS = [
  "list.next",
  "list.open",
  "item.reply",
  "item.done",
  "item.later",
  "conversations.resolve_need",
  "undo",
] as const;

export interface WhatNeedsYouProps {
  workspaceId: string;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/** A conversation with its open need, as Home shows it. */
interface Row {
  item: ItemView;
  conversation: ConversationSummary;
  need: NeedView;
  name: string;
}

/**
 * Home: what needs the member, grouped Review, Reply, Decide, Do, Pay, due
 * soonest first. J/K move, Enter opens, R replies, E is done, L is later,
 * X is not needed, Z undoes. The rows are the conversations module's
 * records; `attention` only says which, why and by when.
 */
export function WhatNeedsYou({ workspaceId, now }: WhatNeedsYouProps) {
  const { t: typedAttention, i18n } = useTranslation("attention");
  const t = typedAttention as unknown as Translate;
  const { t: typedConversations } = useTranslation("conversations");
  const tc = typedConversations as unknown as Translate;
  const { t: tAny } = useTranslation();
  // Shortcut labels are namespaced keys only known at runtime.
  const translateLabel = tAny as unknown as (key: string) => string;
  const { registry } = useShortcuts();
  const navigate = useNavigate();
  const undo = useUndoToast();

  const open = useAttention(workspaceId, "open");
  const cleared = useAttention(workspaceId, "cleared");
  const refresh = useRefreshAttention(workspaceId);
  const needActions = useNeedActions(workspaceId, refresh);
  const snooze = useSnooze(workspaceId);

  // Items acted on, hidden at once while the projection catches up; keyed
  // by their cause, so a follow-up from the sender shows again.
  const [gone, setGone] = useState<ReadonlySet<string>>(new Set());
  const openItems = open.data?.items ?? [];

  const clearedItems = cleared.data?.items ?? [];
  const ids = [
    ...new Set(
      [...openItems, ...clearedItems]
        .filter((item) => item.record.type === CONVERSATION)
        .map((item) => item.record.id),
    ),
  ];
  const conversations = useConversationsByIds(workspaceId, ids);
  const byId = new Map((conversations.data ?? []).map((c) => [c.id, c] as const));

  const rows: Row[] = openItems.flatMap((item) => {
    const conversation = byId.get(item.record.id);
    const need = conversation?.need;
    if (
      gone.has(goneKey(item)) ||
      conversation === undefined ||
      need === null ||
      need === undefined
    ) {
      return [];
    }
    return [{ item, conversation, need, name: needSender(need, conversation) }];
  });
  const groups = groupByReason(rows.map((row) => row.item)).map((group) => ({
    reason: group.reason,
    rows: group.items.flatMap((item) => rows.filter((row) => row.item.id === item.id)),
  }));
  const order = groups.flatMap((group) => group.rows);

  const [selectedId, setSelectedId] = useState<string | undefined>();
  const [expandedId, setExpandedId] = useState<string | undefined>();
  const [laterId, setLaterId] = useState<string | undefined>();
  const selected = order.find((row) => row.item.id === selectedId) ?? order[0];

  const hide = (row: Row) => {
    setGone((current) => new Set([...current, goneKey(row.item)]));
    const index = order.findIndex((r) => r.item.id === row.item.id);
    const next = order[index + 1] ?? order[index - 1];
    setSelectedId(next?.item.id);
    setExpandedId(undefined);
    setLaterId(undefined);
  };
  const show = (row: Row) => {
    setGone((current) => new Set([...current].filter((key) => key !== goneKey(row.item))));
    setSelectedId(row.item.id);
  };
  const isQuestion = (row: Row) => row.need.trust === "check";

  const done = (row: Row) => {
    hide(row);
    needActions.done(row.conversation);
  };
  const notNeeded = (row: Row) => {
    hide(row);
    needActions.notNeeded(row.conversation);
  };
  const confirm = (row: Row) => {
    needActions.confirm(row.conversation);
  };
  const later = (row: Row, until: Date) => {
    hide(row);
    snooze.mutate({ item: row.item.id, until });
    const when = new Intl.DateTimeFormat(i18n.language, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit",
    }).format(until);
    undo.push({
      label: t("later.toast", { name: row.name, when }),
      undo: () => {
        show(row);
        snooze.mutate({ item: row.item.id, until: undefined });
      },
      redo: () => {
        hide(row);
        snooze.mutate({ item: row.item.id, until });
      },
    });
  };
  const reply = (row: Row) => {
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: {
        workspaceId,
        stream: row.conversation.stream,
        conversationId: row.conversation.id,
      },
      search: { reply: true },
    });
  };
  const openConversation = (row: Row) => {
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: {
        workspaceId,
        stream: row.conversation.stream,
        conversationId: row.conversation.id,
      },
    });
  };

  const move = (step: number) => {
    const index = order.findIndex((row) => row.item.id === selected?.item.id);
    const next = order[Math.min(order.length - 1, Math.max(0, index + step))];
    if (next !== undefined) {
      setSelectedId(next.item.id);
      const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
        document.getElementById(itemId(next.item.id));
      // Not every environment scrolls (tests run without layout).
      element?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const hasRows = selected !== undefined;
  useShortcut("list.next", () => {
    move(1);
  });
  useShortcut("list.previous", () => {
    move(-1);
  });
  useShortcut(
    "list.open",
    () => {
      if (selected !== undefined) {
        setExpandedId((id) => (id === selected.item.id ? undefined : selected.item.id));
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.done",
    () => {
      if (selected !== undefined && !isQuestion(selected)) {
        done(selected);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "conversations.resolve_need",
    () => {
      if (selected !== undefined) {
        notNeeded(selected);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.later",
    () => {
      if (selected !== undefined) {
        setExpandedId(selected.item.id);
        setLaterId(selected.item.id);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.reply",
    () => {
      if (selected !== undefined) {
        reply(selected);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.choice1",
    () => {
      if (selected !== undefined && isQuestion(selected)) {
        confirm(selected);
      }
    },
    { enabled: selected !== undefined && isQuestion(selected) },
  );
  useShortcut(
    "item.choice2",
    () => {
      if (selected !== undefined && isQuestion(selected)) {
        notNeeded(selected);
      }
    },
    { enabled: selected !== undefined && isQuestion(selected) },
  );

  const today = now ?? new Date();
  const dateLine = new Intl.DateTimeFormat(i18n.language, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(today);
  const loading = open.isPending || (ids.length > 0 && conversations.isPending);
  const count = order.length;
  const review = order.some((row) => row.item.reason === "review");

  const aside = (
    <>
      <AsideSection title={t("doneTodayTitle")}>
        <DoneToday
          items={clearedItems}
          byId={byId}
          t={t}
          onReopen={(conversation) => {
            needActions.reopen(conversation);
          }}
          onUnsnooze={(item) => {
            snooze.mutate({ item: item.id, until: undefined });
          }}
        />
      </AsideSection>
      <AsideSection title={t("keyboard")}>
        {KEYS.map((id) => {
          const definition = registry.get(id);
          return definition === undefined ? null : (
            <Line key={id} trailing={<KbdHint shortcut={id} announce />}>
              {translateLabel(definition.label)}
            </Line>
          );
        })}
      </AsideSection>
    </>
  );

  if (open.isError) {
    return (
      <HomeLayout asideLabel={t("context")} aside={aside}>
        <EmptyState
          title={t("error.title")}
          visor="thinking"
          action={
            <Button
              onClick={() => {
                void open.refetch();
              }}
            >
              {t("error.retry")}
            </Button>
          }
        >
          {t("error.body")}
        </EmptyState>
      </HomeLayout>
    );
  }

  return (
    <HomeLayout asideLabel={t("context")} aside={aside}>
      <PageHero
        visor={loading ? "reading" : count === 0 ? "happy" : review ? "alert" : "idle"}
        dateLine={dateLine}
        title={count === 0 && !loading ? t("hero.clear") : t("hero.count", { count })}
        subline={count === 0 && !loading ? t("hero.clearSubline") : t("hero.subline")}
      />
      {loading ? <SkeletonList rows={4} /> : null}
      {groups.map((group) => (
        <QueueGroup
          key={group.reason}
          signal={group.reason}
          name={t(`groups.${group.reason}`)}
          count={group.rows.length}
        >
          {group.rows.map((row) => {
            const due = dueLabel(tc, row.need, i18n.language, now);
            const expanded = expandedId === row.item.id;
            return (
              <Item
                key={row.item.id}
                id={itemId(row.item.id)}
                who={row.name}
                what={needSentence(tc, row.need, row.name)}
                excerpt={row.need.quote?.text}
                avatar={{
                  name: row.name,
                  kind:
                    row.item.reason === "pay" || row.need.question === "suspicious"
                      ? "service"
                      : "person",
                }}
                due={due?.text}
                soon={due?.soon === true}
                selected={row.item.id === selected?.item.id}
                expanded={expanded}
                hints={
                  isQuestion(row) ? ["item.choice1", "item.choice2"] : ["item.done", "item.later"]
                }
                onToggle={() => {
                  setSelectedId(row.item.id);
                  setExpandedId(expanded ? undefined : row.item.id);
                }}
                detail={
                  <NeedDetail
                    workspaceId={workspaceId}
                    row={row}
                    t={t}
                    tc={tc}
                    locale={i18n.language}
                    laterOpen={laterId === row.item.id}
                    onLaterOpenChange={(value) => {
                      setLaterId(value ? row.item.id : undefined);
                    }}
                    now={today}
                    onReply={() => {
                      reply(row);
                    }}
                    onDone={() => {
                      done(row);
                    }}
                    onNotNeeded={() => {
                      notNeeded(row);
                    }}
                    onConfirm={() => {
                      confirm(row);
                    }}
                    onLater={(until) => {
                      later(row, until);
                    }}
                    onOpen={() => {
                      openConversation(row);
                    }}
                  />
                }
              />
            );
          })}
        </QueueGroup>
      ))}
    </HomeLayout>
  );
}

const itemId = (id: string) => `attention-${id}`;

const goneKey = (item: ItemView) => `${item.id}:${item.cause.id}`;

interface NeedDetailProps {
  workspaceId: string;
  row: Row;
  t: Translate;
  tc: Translate;
  locale: string;
  laterOpen: boolean;
  onLaterOpenChange: (open: boolean) => void;
  now: Date;
  onReply: () => void;
  onDone: () => void;
  onNotNeeded: () => void;
  onConfirm: () => void;
  onLater: (until: Date) => void;
  onOpen: () => void;
}

/** An opened item: the warning's tells, the sender's words, its actions and how it was decided. */
function NeedDetail({
  workspaceId,
  row,
  t,
  tc,
  locale,
  laterOpen,
  onLaterOpenChange,
  now,
  onReply,
  onDone,
  onNotNeeded,
  onConfirm,
  onLater,
  onOpen,
}: NeedDetailProps) {
  const { i18n } = useTranslation("conversations");
  const exists = (key: string) => i18n.exists(`conversations:${key}`);
  const detail = useConversation(workspaceId, row.conversation.id);
  const decisions = detail.data?.decisions ?? [];
  const { need } = row;
  const warning = need.question === "suspicious";
  const question = need.trust === "check";
  return (
    <ItemDetail
      reasons={warning ? warningReasons(tc, exists, decisions) : []}
      quoteFrom={quoteAttribution(need, row.name, locale)}
      quote={need.quote?.text}
      actions={
        <>
          {question ? (
            <>
              <Button variant="primary" shortcut="item.choice1" onClick={onConfirm}>
                {tc("needs.actions.yes")}
              </Button>
              <Button variant="secondary" shortcut="item.choice2" onClick={onNotNeeded}>
                {tc("needs.actions.no")}
              </Button>
            </>
          ) : (
            <>
              {warning ? null : (
                <Button variant="primary" shortcut="item.reply" onClick={onReply}>
                  {t("actions.reply")}
                </Button>
              )}
              <Button
                variant={warning ? "primary" : "secondary"}
                shortcut="item.done"
                onClick={onDone}
              >
                {tc("needs.actions.done")}
              </Button>
              <Popover
                label={t("later.title")}
                placement="top-start"
                open={laterOpen}
                onOpenChange={onLaterOpenChange}
                renderTrigger={(trigger) => (
                  <Button variant="ghost" shortcut="item.later" {...trigger}>
                    {t("later.title")}
                  </Button>
                )}
              >
                {(close) =>
                  laterOptions(now).map((option) => (
                    <Button
                      key={option.key}
                      variant="ghost"
                      onClick={() => {
                        close();
                        onLater(option.until);
                      }}
                    >
                      {t(`later.${option.key}`)}
                    </Button>
                  ))
                }
              </Popover>
              <Button variant="ghost" shortcut="conversations.resolve_need" onClick={onNotNeeded}>
                {tc(warning ? "needs.actions.safe" : "needs.actions.notNeeded")}
              </Button>
            </>
          )}
          <Button variant="link" onClick={onOpen}>
            {t("actions.open")}
          </Button>
        </>
      }
      how={<HowDecided rows={howRows(tc, exists, decisions)} footnote={tc("needs.how.footnote")} />}
    />
  );
}

interface DoneTodayProps {
  items: readonly ItemView[];
  byId: ReadonlyMap<string, ConversationSummary>;
  t: Translate;
  onReopen: (conversation: ConversationSummary) => void;
  onUnsnooze: (item: ItemView) => void;
}

/** What left the list today, in sentences, with Undo where it can be undone. */
function DoneToday({ items, byId, t, onReopen, onUnsnooze }: DoneTodayProps) {
  const lines = items.flatMap((item) => {
    const conversation = byId.get(item.record.id);
    if (conversation === undefined) {
      return [];
    }
    const subject = conversation.subject === "" ? t("noSubject") : conversation.subject;
    const kind = doneTodayKind(item);
    return [{ item, conversation, kind, text: t(kind.key, { subject }) }];
  });
  if (lines.length === 0) {
    return <LineMeta>{t("doneTodayEmpty")}</LineMeta>;
  }
  return (
    <>
      {lines.map(({ item, conversation, kind, text }) => (
        <Line
          key={item.id}
          trailing={
            kind.undo === undefined ? undefined : (
              <Button
                variant="link"
                onClick={() => {
                  if (kind.undo === "reopen") {
                    onReopen(conversation);
                  } else {
                    onUnsnooze(item);
                  }
                }}
              >
                {t("actions.undo")}
              </Button>
            )
          }
        >
          {text}
        </Line>
      ))}
    </>
  );
}

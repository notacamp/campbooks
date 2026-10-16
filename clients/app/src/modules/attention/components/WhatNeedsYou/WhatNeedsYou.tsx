import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  Dialog,
  DialogChoices,
  EmptyState,
  HomeLayout,
  HowDecided,
  Item,
  ItemActions,
  ItemDetail,
  Line,
  LineMeta,
  PageHero,
  QueueGroup,
  SkeletonList,
  useUndoToast,
  type ItemAction,
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
import {
  doneTodayKind,
  groupByReason,
  rowActionIds,
  snoozeOptions,
  type OwnActionId,
  type UniversalActionId,
} from "../../model";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

const CONVERSATION = "conversations.conversation";

/** The item's own actions take 1, 2 and 3, in order. */
const CHOICE_KEYS = ["item.choice1", "item.choice2", "item.choice3"] as const;

const UNIVERSAL_KEYS: Record<UniversalActionId, string> = {
  done: "item.done",
  snooze: "item.snooze",
  not_needed: "conversations.resolve_need",
};

export interface WhatNeedsYouProps {
  workspaceId: string;
  /** Under the hero: an offer from another module (Scout's questions). */
  offer?: ReactNode | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/** A conversation with its open need, as Home shows it. */
interface Row {
  item: ItemView;
  conversation: ConversationSummary;
  need: NeedView;
  name: string;
  /** Acted on: the one-line confirmation it shows while it folds away. */
  confirmation?: string | undefined;
}

type ActionId = OwnActionId | UniversalActionId;

const kindOf = (need: NeedView) =>
  need.question === "suspicious" ? "warning" : need.trust === "check" ? "question" : "need";

/**
 * Home: what needs the member, grouped Review, Reply, Decide, Do, Pay, due
 * soonest first. The selected item shows its actions under it (its own on
 * 1 to 3, then Done E, Snooze S, Not needed X); ↑/↓ (J/K) move, ←/→ walk
 * the buttons, Enter presses one or opens the item, Esc clears, Z undoes.
 * A row acted on folds away with a one-line confirmation. The rows are the
 * conversations module's records; `attention` only says which, why and by
 * when.
 */
export function WhatNeedsYou({ workspaceId, now, offer }: WhatNeedsYouProps) {
  const { t: typedAttention, i18n } = useTranslation("attention");
  const t = typedAttention as unknown as Translate;
  const { t: typedConversations } = useTranslation("conversations");
  const tc = typedConversations as unknown as Translate;
  const navigate = useNavigate();
  const undo = useUndoToast();

  const open = useAttention(workspaceId, "open");
  const cleared = useAttention(workspaceId, "cleared");
  const refresh = useRefreshAttention(workspaceId);
  const snooze = useSnooze(workspaceId);

  // Items acted on, hidden at once while the projection catches up; keyed
  // by their cause, so a follow-up from the sender shows again. A hidden
  // row with a confirmation still shows it while it folds away.
  const [gone, setGone] = useState<ReadonlyMap<string, Row>>(new Map());
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

  const shown: Row[] = openItems.flatMap((item) => {
    const hidden = gone.get(goneKey(item));
    if (hidden !== undefined) {
      return hidden.confirmation === undefined ? [] : [hidden];
    }
    const conversation = byId.get(item.record.id);
    const need = conversation?.need;
    if (conversation === undefined || need === null || need === undefined) {
      return [];
    }
    return [{ item, conversation, need, name: needSender(need, conversation) }];
  });
  const rows = shown.filter((row) => row.confirmation === undefined);
  const groups = groupByReason(shown.map((row) => row.item)).map((group) => ({
    reason: group.reason,
    rows: group.items.flatMap((item) => shown.filter((row) => row.item.id === item.id)),
  }));
  const order = groups
    .flatMap((group) => group.rows)
    .filter((row) => row.confirmation === undefined);

  // The selection is always an explicit item id, with the position it was
  // last seen at: when a live update takes it away, its neighbour at that
  // position takes over, never the top of the list.
  const [selection, setSelection] = useState<{ id?: string; index: number }>({ index: 0 });
  const [expandedId, setExpandedId] = useState<string | undefined>();
  // The action ←/→ are on, on the selected row.
  const [highlight, setHighlight] = useState<string | undefined>();
  const [snoozing, setSnoozing] = useState<Row | undefined>();
  const foundAt = order.findIndex((row) => row.item.id === selection.id);
  if (order.length > 0 && (foundAt === -1 || foundAt !== selection.index)) {
    // Adjusting state while rendering, so the change never shows a stale frame.
    const index = foundAt === -1 ? Math.min(selection.index, order.length - 1) : foundAt;
    const next = order[index];
    if (next !== undefined && (next.item.id !== selection.id || index !== selection.index)) {
      setSelection({ id: next.item.id, index });
      setHighlight(undefined);
    }
  }
  const selected = order.find((row) => row.item.id === selection.id);

  // Keyboard focus follows the selection when it was on the list (a row or
  // its actions), so a screen reader hears where it went.
  const list = useRef<HTMLDivElement>(null);
  const focusFollows = useRef(false);
  const keepFocus = () => {
    const active = document.activeElement;
    // Inside the Snooze dialog: it was decided when the dialog opened.
    if ((active?.closest("[role='dialog']") ?? null) !== null) {
      return;
    }
    focusFollows.current = active !== null && list.current?.contains(active) === true;
  };
  const selectedId = selected?.item.id;
  useEffect(() => {
    // The dialog holds the focus while it is open; it comes back here after.
    if (
      !focusFollows.current ||
      selectedId === undefined ||
      highlight !== undefined ||
      snoozing !== undefined
    ) {
      return;
    }
    focusFollows.current = false;
    document.getElementById(itemId(selectedId))?.focus({ preventScroll: true });
  }, [selectedId, highlight, snoozing]);

  const select = (id: string) => {
    setSelection({
      id,
      index: Math.max(
        0,
        order.findIndex((row) => row.item.id === id),
      ),
    });
    setHighlight(undefined);
  };

  const hide = (row: Row, confirmation?: string) => {
    keepFocus();
    setGone((current) => new Map([...current, [goneKey(row.item), { ...row, confirmation }]]));
    const index = order.findIndex((r) => r.item.id === row.item.id);
    // The row below takes its place (or the one above, at the end).
    const next = order[index + 1] ?? order[index - 1];
    setSelection({
      ...(next === undefined ? {} : { id: next.item.id }),
      index: order[index + 1] === undefined ? Math.max(0, index - 1) : index,
    });
    setExpandedId(undefined);
    setHighlight(undefined);
  };
  // The confirmation has folded away: the row is simply hidden now.
  const folded = (row: Row) => {
    setGone((current) => {
      const hidden = current.get(goneKey(row.item));
      return hidden?.confirmation === undefined
        ? current
        : new Map([...current, [goneKey(row.item), { ...hidden, confirmation: undefined }]]);
    });
  };
  const show = (row: Row) => {
    setGone((current) => new Map([...current].filter(([key]) => key !== goneKey(row.item))));
    select(row.item.id);
  };
  // A hidden row is found among the hidden ones: its summary may already say
  // it needs nothing.
  const rowOf = (conversation: ConversationSummary) =>
    [...rows, ...gone.values()].find((row) => row.conversation.id === conversation.id);
  const needActions = useNeedActions(workspaceId, {
    onChanged: refresh,
    onUndo: (conversation) => {
      const row = rowOf(conversation);
      if (row !== undefined) {
        show(row);
      }
    },
    onRedo: (conversation) => {
      const row = rowOf(conversation);
      if (row !== undefined) {
        hide(row);
      }
    },
    onFailed: (conversation) => {
      const row = rowOf(conversation);
      if (row !== undefined) {
        show(row);
      }
    },
  });

  const when = (until: Date) =>
    new Intl.DateTimeFormat(i18n.language, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit",
    }).format(until);

  const done = (row: Row) => {
    hide(row, t("confirm.done", { name: row.name }));
    needActions.done(row.conversation);
  };
  const notNeeded = (row: Row) => {
    const warning = kindOf(row.need) === "warning";
    hide(row, t(warning ? "confirm.safe" : "confirm.notNeeded", { name: row.name }));
    needActions.notNeeded(row.conversation);
  };
  const confirm = (row: Row) => {
    needActions.confirm(row.conversation);
  };
  // Each request settles on its own promise (`mutate`'s callbacks only fire
  // for the latest call). `entry` is the snooze's undo entry, taken back when
  // it fails.
  const snoozeOrRollBack = (row: Row, until?: Date, entry?: number) => {
    void snooze.mutateAsync({ item: row.item.id, until }).catch(() => {
      if (until === undefined) {
        hide(row);
        undo.fail({ label: t("snooze.undoFailed", { name: row.name }) });
        return;
      }
      if (entry !== undefined) {
        undo.retract(entry);
      }
      show(row);
      undo.fail({
        label: t("snooze.failed", { name: row.name }),
        retry: () => {
          snoozeUntil(row, until);
        },
      });
    });
  };
  const snoozeUntil = (row: Row, until: Date) => {
    hide(row, t("confirm.snoozed", { name: row.name, when: when(until) }));
    const entry = undo.push({
      label: t("snooze.toast", { name: row.name, when: when(until) }),
      undo: () => {
        show(row);
        snoozeOrRollBack(row);
      },
      redo: () => {
        hide(row);
        snoozeOrRollBack(row, until);
      },
    });
    snoozeOrRollBack(row, until, entry);
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

  /** What a row offers, in words with keys: the shape #166's ranked actions drop into. */
  const actionsOf = (row: Row): { own: ItemAction[]; universal: ItemAction[] } => {
    const ids = rowActionIds(kindOf(row.need));
    const ownLabel: Record<OwnActionId, string> = {
      reply: t("actions.reply"),
      yes: tc("needs.actions.yes"),
      no: tc("needs.actions.no"),
      safe: tc("needs.actions.safe"),
    };
    const universalLabel: Record<UniversalActionId, string> = {
      done: tc("needs.actions.done"),
      snooze: t("snooze.button"),
      not_needed: tc("needs.actions.notNeeded"),
    };
    return {
      own: ids.own.slice(0, CHOICE_KEYS.length).map((id, index) => ({
        id,
        label: ownLabel[id],
        key: CHOICE_KEYS[index] ?? "item.choice1",
      })),
      universal: ids.universal.map((id) => ({
        id,
        label: universalLabel[id],
        key: UNIVERSAL_KEYS[id],
      })),
    };
  };
  const startSnooze = (row: Row) => {
    keepFocus();
    setHighlight(undefined);
    setSnoozing(row);
  };
  const run = (row: Row, id: ActionId) => {
    switch (id) {
      case "reply": {
        reply(row);
        return;
      }
      case "yes": {
        setHighlight(undefined);
        confirm(row);
        return;
      }
      case "no":
      case "safe":
      case "not_needed": {
        notNeeded(row);
        return;
      }
      case "done": {
        done(row);
        return;
      }
      case "snooze": {
        startSnooze(row);
        return;
      }
    }
  };
  const selectedActions = selected === undefined ? undefined : actionsOf(selected);
  const offered = (id: ActionId) =>
    selectedActions !== undefined &&
    [...selectedActions.own, ...selectedActions.universal].some((action) => action.id === id);

  const move = (step: number) => {
    keepFocus();
    const index = order.findIndex((row) => row.item.id === selected?.item.id);
    const target = Math.min(order.length - 1, Math.max(0, index + step));
    const next = order[target];
    if (next !== undefined) {
      setSelection({ id: next.item.id, index: target });
      setHighlight(undefined);
      if (next.item.id !== selected?.item.id) {
        setExpandedId(undefined);
      }
      const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
        document.getElementById(itemId(next.item.id));
      // Not every environment scrolls (tests run without layout).
      element?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const walk = (step: 1 | -1) => {
    if (selectedActions === undefined) {
      return;
    }
    const all = [...selectedActions.own, ...selectedActions.universal];
    const at = all.findIndex((action) => action.id === highlight);
    const next =
      at === -1
        ? step === 1
          ? 0
          : all.length - 1
        : Math.max(0, Math.min(all.length - 1, at + step));
    setHighlight(all[next]?.id);
  };
  const hasRows = selected !== undefined;
  useShortcut("list.next", () => {
    move(1);
  });
  useShortcut("list.previous", () => {
    move(-1);
  });
  useShortcut(
    "item.next_action",
    () => {
      walk(1);
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.previous_action",
    () => {
      walk(-1);
    },
    { enabled: hasRows },
  );
  useShortcut(
    "list.open",
    (event) => {
      // Enter on another control (the toast's Undo or Retry, Done today's
      // Undo, Open conversation) is that control's: press it, as the browser
      // would have. Only a row, an action button or the page itself is Home's.
      const target = event?.target;
      if (target instanceof HTMLElement && target !== document.body) {
        const row = target.id.startsWith(ITEM_ID_PREFIX);
        const action = target.dataset["action"] !== undefined;
        if (!row && !action) {
          if (target.matches("button, a[href]")) {
            target.click();
          }
          return;
        }
      }
      if (selected === undefined) {
        return;
      }
      if (highlight === undefined) {
        setExpandedId((id) => (id === selected.item.id ? undefined : selected.item.id));
      } else {
        run(selected, highlight as ActionId);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "overlay.close",
    () => {
      keepFocus();
      setHighlight(undefined);
      setExpandedId(undefined);
    },
    { enabled: highlight !== undefined || expandedId !== undefined },
  );
  const own = (index: number) => () => {
    const action = selectedActions?.own[index];
    if (selected !== undefined && action !== undefined) {
      run(selected, action.id as ActionId);
    }
  };
  useShortcut("item.choice1", own(0), { enabled: selectedActions?.own[0] !== undefined });
  useShortcut("item.choice2", own(1), { enabled: selectedActions?.own[1] !== undefined });
  useShortcut("item.choice3", own(2), { enabled: selectedActions?.own[2] !== undefined });
  useShortcut(
    "item.done",
    () => {
      if (selected !== undefined) {
        done(selected);
      }
    },
    { enabled: offered("done") },
  );
  useShortcut(
    "item.snooze",
    () => {
      if (selected !== undefined) {
        startSnooze(selected);
      }
    },
    { enabled: offered("snooze") },
  );
  // X: not needed, a warning's "It's safe" or a question's No.
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
    "item.reply",
    () => {
      if (selected !== undefined) {
        reply(selected);
      }
    },
    { enabled: offered("reply") },
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
  const times = snoozeOptions(today);

  const aside = (
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
        title={loading ? t("title") : count === 0 ? t("hero.clear") : t("hero.count", { count })}
        subline={count === 0 && !loading ? t("hero.clearSubline") : t("hero.subline")}
      />
      {offer}
      {loading ? <SkeletonList rows={4} /> : null}
      <div ref={list} className="flex flex-col">
        {groups.map((group) => (
          <QueueGroup
            key={group.reason}
            signal={group.reason}
            name={t(`groups.${group.reason}`)}
            count={group.rows.filter((row) => row.confirmation === undefined).length}
          >
            {group.rows.map((row) => {
              const due = dueLabel(tc, row.need, i18n.language, now);
              const expanded = expandedId === row.item.id;
              const isSelected = row.item.id === selected?.item.id;
              const actions = isSelected ? actionsOf(row) : undefined;
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
                  selected={isSelected}
                  expanded={expanded}
                  confirmation={row.confirmation}
                  onConfirmEnd={() => {
                    folded(row);
                  }}
                  onToggle={() => {
                    select(row.item.id);
                    setExpandedId(expanded ? undefined : row.item.id);
                  }}
                  actions={
                    actions === undefined ? undefined : (
                      <ItemActions
                        label={t("actions.label", { name: row.name })}
                        own={actions.own}
                        universal={actions.universal}
                        highlighted={highlight}
                        onHighlight={setHighlight}
                        onAction={(id) => {
                          run(row, id as ActionId);
                        }}
                      />
                    )
                  }
                  detail={
                    <NeedDetail
                      workspaceId={workspaceId}
                      row={row}
                      t={t}
                      tc={tc}
                      locale={i18n.language}
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
      </div>
      <Dialog
        open={snoozing !== undefined}
        onClose={() => {
          setSnoozing(undefined);
        }}
        title={t("snooze.title", { name: snoozing?.name ?? "" })}
        description={t("snooze.description")}
      >
        <DialogChoices
          label={t("snooze.choices")}
          choices={times.map((option) => ({
            id: option.key,
            label: t(`snooze.${option.key}`),
            detail: when(option.until),
          }))}
          onChoose={(key) => {
            const row = snoozing;
            const option = times.find((o) => o.key === key);
            setSnoozing(undefined);
            if (row !== undefined && option !== undefined) {
              snoozeUntil(row, option.until);
            }
          }}
        />
      </Dialog>
    </HomeLayout>
  );
}

const ITEM_ID_PREFIX = "attention-";
const itemId = (id: string) => `${ITEM_ID_PREFIX}${id}`;

const goneKey = (item: ItemView) => `${item.id}:${item.cause.id}`;

interface NeedDetailProps {
  workspaceId: string;
  row: Row;
  t: Translate;
  tc: Translate;
  locale: string;
  onOpen: () => void;
}

/** An opened item: the warning's tells, the sender's words and how it was decided. */
function NeedDetail({ workspaceId, row, t, tc, locale, onOpen }: NeedDetailProps) {
  const { i18n } = useTranslation("conversations");
  const exists = (key: string) => i18n.exists(`conversations:${key}`);
  const detail = useConversation(workspaceId, row.conversation.id);
  const decisions = detail.data?.decisions ?? [];
  const { need } = row;
  const warning = need.question === "suspicious";
  return (
    <ItemDetail
      reasons={warning ? warningReasons(tc, exists, decisions) : []}
      quoteFrom={quoteAttribution(need, row.name, locale)}
      quote={need.quote?.text}
      actions={
        <Button variant="link" onClick={onOpen}>
          {t("actions.open")}
        </Button>
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

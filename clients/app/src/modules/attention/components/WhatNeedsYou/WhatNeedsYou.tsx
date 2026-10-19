import { useNavigate } from "@tanstack/react-router";
import { Fragment, useEffect, useEffectEvent, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  Dialog,
  DialogChoices,
  EmptyState,
  focusNextAttachment,
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
  billKind,
  dueLabel,
  ForwardDialog,
  howRows,
  NeedMessage,
  needMessageOf,
  needSender,
  needSentence,
  quoteAttribution,
  ReplyDialog,
  ScoutNoticed,
  useAheadOffer,
  useConversation,
  useConversationsByIds,
  useInboxes,
  useNeedActions,
  warningReasons,
  type ConversationSummary,
  type NeedView,
  type ReplyResume,
} from "~/modules/conversations";

import { useAttention, useRefreshAttention, useSnooze, type ItemView } from "../../api/queries";
import {
  ageLabel,
  doneTodayKind,
  groupByReason,
  remindTime,
  rowChoices,
  signalOf,
  snoozeOptions,
  type OwnActionId,
  type OwnChoice,
  type RowKind,
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
  /** Where the item stood in the list when it was acted on. */
  at?: number | undefined;
}

type ActionId = OwnActionId | UniversalActionId;

/** The steps that open a dialog: a pair runs at most one of them. */
const DIALOG_STEPS: ReadonlySet<ActionId> = new Set(["reply", "remind_when", "forward", "snooze"]);

/** What an own action needs to work in an inbox the member may only read. */
const READ_ONLY_STEPS: ReadonlySet<OwnActionId> = new Set(["remind_when", "open_attachment"]);

const kindOf = (need: NeedView): RowKind => {
  if (need.question === "suspicious") {
    return "warning";
  }
  // A rule the member taught keeps it here until they open it: no question.
  if (need.question === "worth_a_look") {
    return "highlight";
  }
  // A bill: asked "Is it paid?" until the member answers, then "Pay …".
  const bill = billKind(need);
  if (bill !== undefined) {
    return bill === "ask" ? "bill" : "toPay";
  }
  return need.trust === "check" ? "question" : "need";
};

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

  // A row still folding away keeps its place even once the list no longer
  // has its item (the server answered and the list refetched first).
  const present = new Set(openItems.map(goneKey));
  const listed = [...openItems];
  for (const row of gone.values()) {
    if (row.confirmation !== undefined && !present.has(goneKey(row.item))) {
      listed.splice(Math.min(row.at ?? listed.length, listed.length), 0, row.item);
    }
  }
  const shown: Row[] = listed.flatMap((item) => {
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
  // Snooze's dialog, or Remind's (the same times, a bill's due date first).
  const [snoozing, setSnoozing] = useState<{ row: Row; remind: boolean } | undefined>();
  // Forward's dialog: whom to send it to.
  const [forwarding, setForwarding] = useState<Row | undefined>();
  // Open attachment: the opened item whose first file takes the focus.
  const [attachmentFor, setAttachmentFor] = useState<string | undefined>();
  // The reply dialog's item, and the draft to pick up again after an undone send.
  const [replying, setReplying] = useState<
    { row: Row; resume?: ReplyResume | undefined; opened: number } | undefined
  >();
  const replies = useRef(0);
  const inboxes = useInboxes(workspaceId).data;
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
    // Inside a dialog (Snooze, Reply): it was decided when the dialog opened.
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
      snoozing !== undefined ||
      replying !== undefined ||
      forwarding !== undefined
    ) {
      return;
    }
    focusFollows.current = false;
    document.getElementById(itemId(selectedId))?.focus({ preventScroll: true });
  }, [selectedId, highlight, snoozing, replying, forwarding]);

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
    const at = openItems.findIndex((item) => goneKey(item) === goneKey(row.item));
    setGone((current) => new Map([...current, [goneKey(row.item), { ...row, confirmation, at }]]));
    const index = order.findIndex((r) => r.item.id === row.item.id);
    // The row below takes its place (or the one above, at the end).
    const next = order[index + 1] ?? order[index - 1];
    setSelection({
      ...(next === undefined ? {} : { id: next.item.id }),
      index: order[index + 1] === undefined ? Math.max(0, index - 1) : index,
    });
    // Reading item by item: the one taking its place opens too.
    setExpandedId((open) => (open === row.item.id ? next?.item.id : undefined));
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
  const ahead = useAheadOffer(workspaceId);
  const needActions = useNeedActions(workspaceId, {
    onChanged: refresh,
    onUndo: (conversation) => {
      // Undone (Already paid, Report and delete…): no offer about it.
      ahead.cancel(conversation.id);
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
    // The smart rule for mail still to come (campbooks-cloud#169): after
    // Already paid, the sender's bills; after Report and delete, mail like
    // it. Only once the server agreed; the confirmation alone saves.
    onPaid: (conversation) => {
      ahead.offer(conversation, { kind: "paid" });
    },
    onReported: (conversation) => {
      ahead.offer(conversation, { kind: "report" });
    },
  });

  const when = (until: Date) =>
    new Intl.DateTimeFormat(i18n.language, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit",
    }).format(until);

  const done = (row: Row) => {
    hide(
      row,
      t(kindOf(row.need) === "highlight" ? "confirm.seen" : "confirm.done", { name: row.name }),
    );
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
  const report = (row: Row) => {
    hide(row, t("confirm.reported", { name: row.name }));
    needActions.report(row.conversation);
  };
  const paid = (row: Row) => {
    hide(row, t("confirm.paid", { name: row.name }));
    needActions.alreadyPaid(row.conversation);
  };
  // Still to pay: the row stays, now "Pay …", with a reminder first.
  const stillToPay = (row: Row) => {
    setHighlight(undefined);
    needActions.stillToPay(row.conversation);
  };
  // Every bill of the Pay group still asked about, with one Undo.
  const allPaid = (bills: readonly Row[]) => {
    for (const row of bills) {
      hide(row, t("confirm.paid", { name: row.name }));
    }
    needActions.allPaid(bills.map((row) => row.conversation));
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
  /** The plain composer at the end of the conversation ("Write my own", "Open in composer"). */
  // The composer on the item's email: on the message a draft answers when
  // one is given (it may not be the newest), else the newest.
  const openComposer = (row: Row, messageId?: string | null) => {
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: {
        workspaceId,
        stream: row.conversation.stream,
        conversationId: row.conversation.id,
      },
      search:
        messageId === undefined || messageId === null
          ? { reply: true }
          : { reply: true, to: messageId },
    });
  };
  // Reply: Scout's likely replies and draft, in a dialog over Home.
  const reply = (row: Row, resume?: ReplyResume) => {
    keepFocus();
    setHighlight(undefined);
    replies.current += 1;
    setReplying({ row, resume, opened: replies.current });
  };
  // Whether the member may write in the item's inbox (not a reader's, not
  // one kept read-only). Until the inboxes are known, as before.
  const writable = (row: Row) => {
    if (inboxes === undefined) {
      return true;
    }
    const inbox = inboxes.find((candidate) => candidate.id === row.conversation.inbox_id);
    return inbox !== undefined && !inbox.read_only && inbox.role !== "reader";
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

  const reminder = (row: Row) => remindTime(row.need.due_on, now ?? new Date());
  // Said from the time it gives: today or tomorrow at 09:00, else the date.
  const remindLabel = (row: Row) => {
    const { until } = reminder(row);
    const from = now ?? new Date();
    const days = Math.round(
      (new Date(until).setHours(0, 0, 0, 0) - new Date(from).setHours(0, 0, 0, 0)) / 86_400_000,
    );
    const time = new Intl.DateTimeFormat(i18n.language, {
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    }).format(until);
    if (days <= 1) {
      return t(days <= 0 ? "remind.today" : "remind.tomorrow", { time });
    }
    const date = new Intl.DateTimeFormat(i18n.language, { day: "numeric", month: "long" }).format(
      until,
    );
    return t("remind.on", { date });
  };

  /**
   * What a row offers, in words with keys: the server's ranked actions when
   * it sent them (campbooks-cloud#166), else the fixed ones.
   */
  const actionsOf = (
    row: Row,
  ): { own: ItemAction[]; universal: ItemAction[]; choices: OwnChoice[]; picked: boolean } => {
    const kind = kindOf(row.need);
    const all = rowChoices(kind, row.need.actions);
    // An inbox the member may only read: nothing that writes there (Reply,
    // Yes, No, It's safe, Already paid, Forward, Done, Not needed); Snooze,
    // Remind and Open attachment are the member's own.
    const choices = writable(row)
      ? all.own
      : all.own.filter((choice) => choice.steps.every((step) => READ_ONLY_STEPS.has(step)));
    const universal = writable(row) ? all.universal : all.universal.filter((id) => id === "snooze");
    const ownLabel = (id: OwnActionId): string => {
      switch (id) {
        case "reply": {
          return t("actions.reply");
        }
        case "yes": {
          return tc("needs.actions.yes");
        }
        case "no": {
          return tc("needs.actions.no");
        }
        case "report": {
          return tc("needs.actions.report");
        }
        case "safe": {
          return tc("needs.actions.safe");
        }
        case "paid": {
          return tc(kind === "bill" ? "needs.actions.alreadyPaid" : "needs.actions.markPaid");
        }
        case "to_pay": {
          return tc("needs.actions.stillToPay");
        }
        case "remind": {
          return remindLabel(row);
        }
        case "remind_when": {
          return t("remind.button");
        }
        case "open_attachment": {
          return t("actions.openAttachment");
        }
        case "forward": {
          return t("actions.forward");
        }
      }
    };
    const choiceLabel = (choice: OwnChoice): string => {
      const [first, second] = choice.steps.map(ownLabel);
      return second === undefined
        ? (first ?? "")
        : t("actions.combo", { first: first ?? "", second });
    };
    const universalLabel: Record<UniversalActionId, string> = {
      done: tc(kind === "highlight" ? "needs.actions.seen" : "needs.actions.done"),
      snooze: t("snooze.button"),
      not_needed: tc("needs.actions.notNeeded"),
    };
    const own = choices.slice(0, CHOICE_KEYS.length);
    return {
      own: own.map((choice, index) => ({
        id: choice.id,
        label: choiceLabel(choice),
        // Reply asks Scout for likely replies: the visor marks it.
        ai: choice.steps.includes("reply"),
        key: CHOICE_KEYS[index] ?? "item.choice1",
      })),
      universal: universal.map((id) => ({
        id,
        label: universalLabel[id],
        key: UNIVERSAL_KEYS[id],
      })),
      choices: own,
      picked: all.picked && own.length > 0,
    };
  };
  const startSnooze = (row: Row, remind = false) => {
    keepFocus();
    setHighlight(undefined);
    setSnoozing({ row, remind });
  };
  // Open attachment: the item opens on the email, its first file focused
  // (Enter previews or saves it, as in the reader). Before a dialog of the
  // same pair, it only opens: the focus stays in the dialog.
  const showAttachment = (row: Row, focus = true) => {
    setHighlight(undefined);
    setExpandedId(row.item.id);
    setAttachmentFor(focus ? row.item.id : undefined);
  };
  const forward = (row: Row) => {
    keepFocus();
    setHighlight(undefined);
    setForwarding(row);
  };
  const step = (row: Row, id: ActionId) => {
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
      case "report": {
        report(row);
        return;
      }
      case "paid": {
        paid(row);
        return;
      }
      case "to_pay": {
        stillToPay(row);
        return;
      }
      case "remind": {
        snoozeUntil(row, reminder(row).until);
        return;
      }
      case "remind_when": {
        startSnooze(row, true);
        return;
      }
      case "open_attachment": {
        showAttachment(row);
        return;
      }
      case "forward": {
        forward(row);
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
  /**
   * An action of the row: one of its own (a pair runs both, in order, with
   * at most one dialog) or a universal one.
   */
  const run = (row: Row, id: string) => {
    const choice = actionsOf(row).choices.find((c) => c.id === id);
    if (choice !== undefined) {
      const dialog = choice.steps.find((each) => DIALOG_STEPS.has(each));
      for (const each of choice.steps) {
        if (each === "open_attachment") {
          showAttachment(row, dialog === undefined);
        } else if (!DIALOG_STEPS.has(each) || each === dialog) {
          step(row, each);
        }
      }
      return;
    }
    if (id === "done" || id === "snooze" || id === "not_needed") {
      step(row, id);
    }
  };
  const selectedActions = selected === undefined ? undefined : actionsOf(selected);
  // Offered alone or within a pair (R presses Reply wherever it sits).
  const offered = (id: ActionId) =>
    selectedActions !== undefined &&
    (selectedActions.choices.some((choice) => {
      const steps: readonly ActionId[] = choice.steps;
      return steps.includes(id);
    }) ||
      selectedActions.universal.some((action) => action.id === id));

  const move = (step: number) => {
    keepFocus();
    const index = order.findIndex((row) => row.item.id === selected?.item.id);
    const target = Math.min(order.length - 1, Math.max(0, index + step));
    const next = order[target];
    if (next !== undefined) {
      setSelection({ id: next.item.id, index: target });
      setHighlight(undefined);
      // The row stepped onto opens, as a click on it does.
      setExpandedId(next.item.id);
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
        run(selected, highlight);
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
    { enabled: (highlight !== undefined || expandedId !== undefined) && !ahead.offering },
  );
  const own = (index: number) => () => {
    const action = selectedActions?.own[index];
    if (selected !== undefined && action !== undefined) {
      run(selected, action.id);
    }
  };
  // An offered smart rule holds 1 and Esc while it shows.
  useShortcut("item.choice1", own(0), {
    enabled: selectedActions?.own[0] !== undefined && !ahead.offering,
  });
  useShortcut("item.choice2", own(1), {
    enabled: selectedActions?.own[1] !== undefined && !ahead.offering,
  });
  useShortcut("item.choice3", own(2), {
    enabled: selectedActions?.own[2] !== undefined && !ahead.offering,
  });
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
    { enabled: offered("not_needed") || offered("no") || offered("safe") },
  );
  // Shift+E: every bill the Pay group asks about is already paid.
  const askedBills = order.filter((row) => kindOf(row.need) === "bill");
  useShortcut(
    "item.all_paid",
    () => {
      allPaid(askedBills);
    },
    { enabled: askedBills.length > 1 },
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
  // Remind's times: a bill's reminder before its due date first, then Snooze's.
  const billDue = (row: Row) => ["bill", "toPay"].includes(kindOf(row.need));
  const beforeDue =
    snoozing?.remind === true && billDue(snoozing.row) && reminder(snoozing.row).kind === "before"
      ? [{ key: "due" as const, until: reminder(snoozing.row).until }]
      : [];
  const asked: { key: string; until: Date }[] = [
    ...beforeDue,
    ...times.filter((option) =>
      beforeDue.every((due) => due.until.getTime() !== option.until.getTime()),
    ),
  ];

  // Scout noticed sits right after the first group, so it is met before the
  // list is long (campbooks-cloud#274); with nothing that needs the member,
  // it is where the list would be. Never while the list loads.
  const noticed = loading ? null : <ScoutNoticed workspaceId={workspaceId} />;

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
        {groups.map((group, position) => {
          const live = group.rows.filter((row) => row.confirmation === undefined);
          const bills = live.filter((row) => kindOf(row.need) === "bill");
          return (
            <Fragment key={group.reason}>
              <QueueGroup
                signal={signalOf(group.reason)}
                name={t(`groups.${group.reason}`)}
                count={live.length}
                note={bills.length > 0 ? t("pay.note") : undefined}
                action={
                  bills.length > 1 ? (
                    <Button
                      shortcut="item.all_paid"
                      onClick={() => {
                        allPaid(bills);
                      }}
                    >
                      {t("pay.allPaid", { count: bills.length })}
                    </Button>
                  ) : undefined
                }
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
                      age={ageLabel(row.item.since, now ?? new Date(), i18n.language)}
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
                            picked={actions.picked ? t("actions.picked") : undefined}
                            onAction={(id) => {
                              run(row, id);
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
                          now={now}
                          focusAttachment={attachmentFor === row.item.id}
                          onAttachmentFocused={(found) => {
                            setAttachmentFor(undefined);
                            // Nothing to focus (only images its body shows):
                            // say so rather than do nothing.
                            if (!found) {
                              undo.notify(t("actions.noAttachment"));
                            }
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
              {position === 0 ? noticed : null}
            </Fragment>
          );
        })}
        {groups.length === 0 ? noticed : null}
      </div>
      {ahead.element}
      <Dialog
        open={snoozing !== undefined}
        onClose={() => {
          setSnoozing(undefined);
        }}
        title={t(snoozing?.remind === true ? "remind.title" : "snooze.title", {
          name: snoozing?.row.name ?? "",
        })}
        description={t(snoozing?.remind === true ? "remind.description" : "snooze.description")}
      >
        <DialogChoices
          label={t("snooze.choices")}
          choices={asked.map((option) => ({
            id: option.key,
            label: t(option.key === "due" ? "remind.beforeDue" : `snooze.${option.key}`),
            detail: when(option.until),
          }))}
          onChoose={(key) => {
            const row = snoozing?.row;
            const option = asked.find((o) => o.key === key);
            setSnoozing(undefined);
            if (row !== undefined && option !== undefined) {
              snoozeUntil(row, option.until);
            }
          }}
        />
      </Dialog>
      {forwarding === undefined ? null : (
        <ForwardDialog
          workspaceId={workspaceId}
          conversationId={forwarding.conversation.id}
          need={forwarding.need}
          name={forwarding.name}
          onClose={() => {
            setForwarding(undefined);
          }}
          onForwarded={() => {
            // The draft waits in the reader, addressed, to read before it goes.
            setForwarding(undefined);
            openConversation(forwarding);
          }}
        />
      )}
      {replying === undefined ? null : (
        <ReplyDialog
          key={replying.opened}
          workspaceId={workspaceId}
          conversationId={replying.row.conversation.id}
          name={replying.row.name}
          resume={replying.resume}
          onClose={() => {
            setReplying(undefined);
          }}
          onWriteOwn={() => {
            setReplying(undefined);
            openComposer(replying.row);
          }}
          onOpenDraft={(draft) => {
            setReplying(undefined);
            openComposer(replying.row, draft.message_id);
          }}
          onSent={() => {
            hide(replying.row, t("confirm.replied", { name: replying.row.name }));
          }}
          onUndone={(resume) => {
            show(replying.row);
            reply(replying.row, resume);
          }}
        />
      )}
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
  now?: Date | undefined;
  /** Open attachment: move the focus to the email's first file once it shows. */
  focusAttachment?: boolean | undefined;
  /** Whether a file took the focus (none: the email has no file to open). */
  onAttachmentFocused?: ((found: boolean) => void) | undefined;
  onOpen: () => void;
}

/** An opened item: the warning's tells, the email itself and how it was decided. */
function NeedDetail({
  workspaceId,
  row,
  t,
  tc,
  locale,
  now,
  focusAttachment = false,
  onAttachmentFocused,
  onOpen,
}: NeedDetailProps) {
  const { i18n } = useTranslation("conversations");
  const exists = (key: string) => i18n.exists(`conversations:${key}`);
  const detail = useConversation(workspaceId, row.conversation.id);
  const decisions = detail.data?.decisions ?? [];
  const { need } = row;
  const warning = need.question === "suspicious";
  const box = useRef<HTMLDivElement>(null);
  const loaded = detail.data !== undefined;
  const focused = useEffectEvent((found: boolean) => {
    onAttachmentFocused?.(found);
  });
  useEffect(() => {
    if (!focusAttachment || !loaded) {
      return;
    }
    // After the email has drawn its files; the clamp opens when one is focused.
    const frame = requestAnimationFrame(() => {
      // Never from behind a dialog.
      const behind = document.querySelector("[role='dialog']") !== null;
      focused(behind || (box.current !== null && focusNextAttachment(box.current)));
    });
    return () => {
      cancelAnimationFrame(frame);
    };
  }, [focusAttachment, loaded]);
  return (
    <div ref={box} className="contents">
      <ItemDetail
        reasons={
          warning ? [...warningReasons(tc, exists, decisions), tc("needs.mail.linksOff")] : []
        }
        summaryLabel={tc("needs.summary.heading")}
        summary={detail.data?.need_summary ?? undefined}
        mail={
          needMessageOf(detail.data?.messages ?? [], need) === undefined ? undefined : (
            <NeedMessage
              workspaceId={workspaceId}
              conversation={row.conversation}
              need={need}
              now={now}
            />
          )
        }
        quoteFrom={quoteAttribution(need, row.name, locale)}
        quote={need.quote?.text}
        actions={
          <Button variant="link" onClick={onOpen}>
            {t("actions.open")}
          </Button>
        }
        how={
          <HowDecided rows={howRows(tc, exists, decisions)} footnote={tc("needs.how.footnote")} />
        }
      />
    </div>
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

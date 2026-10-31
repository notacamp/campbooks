import { useNavigate } from "@tanstack/react-router";
import {
  Fragment,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";

import { conversationRef, useScoutChat } from "~/lib/scout-chat";
import { useShortcut } from "~/lib/shortcuts";
import {
  AsideSection,
  Button,
  ButtonLink,
  Dialog,
  DialogChoices,
  Disclosure,
  EmptyState,
  focusNextAttachment,
  HomeLayout,
  HowDecided,
  Item,
  ItemActions,
  ItemDetail,
  KindDot,
  Line,
  LineMeta,
  PageHero,
  QueueGroup,
  SkeletonList,
  useUndoToast,
  type ItemAction,
  type ItemEyebrow,
} from "~/lib/ui";
import {
  awaitedOf,
  billKind,
  displayName,
  ForwardDialog,
  howRows,
  NeedMessage,
  needMessageOf,
  needSender,
  needSentence,
  NudgeDialog,
  quoteAttribution,
  ReplyDialog,
  ScoutNoticed,
  unsureNote,
  unsureOf,
  useAheadOffer,
  useConversation,
  useConversationsByIds,
  useInboxes,
  useNeedActions,
  warningReasons,
  type ConversationSummary,
  type NeedView,
  type NudgeResume,
  type ReplyResume,
} from "~/modules/conversations";
import {
  useHomeDay,
  useInvitationAnswers,
  useInvitations,
  type Answer,
  type DayEvent,
  type DayLine,
  type InvitationRef,
  type InvitationRow,
} from "~/modules/schedule";
import {
  heldText,
  holdable,
  HoldTime,
  replyWhen,
  useTaskActions,
  useTasksByIds,
  type HoldSources,
  type TaskView,
} from "~/modules/tasks";

import { useAttention, useRefreshAttention, useSnooze, type ItemView } from "../../api/queries";
import {
  ageLabel,
  bandOf,
  doneTodayKind,
  groupByBand,
  isPressing,
  reasonSentence,
  remindTime,
  rowChoices,
  signalOf,
  snoozeOptions,
  type OwnActionId,
  type OwnChoice,
  type RowKind,
  type UniversalActionId,
} from "../../model";

import { WaitingOnOthers, type WaitingRow } from "./WaitingOnOthers";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

const CONVERSATION = "conversations.conversation";
const TASK = "tasks.task";
/** An invitation awaiting the member's answer is an item about its event (campbooks-cloud#360). */
const INVITATION = "schedule.event";

/** Continue with Scout: its action id and its key (`.`, the assistant's). */
const SCOUT = "scout";
const SCOUT_KEY = "scout.ask";

/** The item's own actions take 1, 2 and 3, in order. */
const CHOICE_KEYS = ["item.choice1", "item.choice2", "item.choice3"] as const;

/** A task asking "You replied. Is this done?" (campbooks-cloud#396): Not yet, and the thread. */
const NOT_YET = "not_yet";
const THREAD = "thread";
const THREAD_KEY = "item.open";

/** A Do task's Hold time, and its key. */
const HOLD = "hold";
const HOLD_KEY = "item.hold";

const UNIVERSAL_KEYS: Record<UniversalActionId, string> = {
  done: "item.done",
  snooze: "item.snooze",
  not_needed: "conversations.resolve_need",
};

export interface WhatNeedsYouProps {
  workspaceId: string;
  /** Under the hero: an offer from another module (Scout's questions). */
  offer?: ReactNode | undefined;
  /** The member's free time and calendars, for a Do task's Hold time (the schedule module's). */
  holdSources?: HoldSources | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

/** A conversation with its open need, as Home shows it. */
interface Row {
  item: ItemView;
  conversation: ConversationSummary;
  need: NeedView;
  /** Who it is from; for a nudge, who has not written yet. */
  name: string;
  /** Acted on: the one-line confirmation it shows while it folds away. */
  confirmation?: string | undefined;
  /** Where the item stood in the list when it was acted on. */
  at?: number | undefined;
}

/**
 * A task as Home shows it (campbooks-cloud#43): beside the conversations in
 * Do and Pay, in the same order by due date. Its actions are Done (Paid),
 * Later and Not needed; the tasks module does the work.
 */
interface TaskEntry {
  item: ItemView;
  task: TaskView;
  /** Where it came from, when it came from a conversation. */
  conversation?: ConversationSummary | undefined;
  /** Acted on: the one-line confirmation it shows while it folds away. */
  confirmation?: string | undefined;
  /** Where the item stood in the list when it was acted on. */
  at?: number | undefined;
}

/**
 * An invitation awaiting the member's answer (campbooks-cloud#360): a Decide
 * row whose answers (Accept, Decline, Maybe) the schedule module gives.
 */
interface InviteEntry {
  item: ItemView;
  invite: InvitationRow;
  /** Who asked. */
  name: string;
  /** Acted on: the one-line confirmation it shows while it folds away. */
  confirmation?: string | undefined;
  /** Where the item stood in the list when it was acted on. */
  at?: number | undefined;
}

/** What Home lists: a conversation's need, a task, or an invitation. */
type Entry = Row | TaskEntry | InviteEntry;

/** What Snooze puts off from Home's dialog: a need or an invitation. */
type Snoozable = Row | InviteEntry;

/**
 * An event of the member's calendar among the rows (campbooks-cloud#359):
 * Next up, or one of today's at the end of Today. Never counted, never acted
 * on like a need: the keys move onto it, Enter opens it in Calendar.
 */
interface EventEntry {
  /** `event:<event key>`, so it never meets an attention item's id. */
  id: string;
  event: DayEvent;
}

/** What the keys move through: the needs and tasks, and the calendar's rows. */
type Listed = Entry | EventEntry;

const EVENT_PREFIX = "event:";

const isTask = (entry: Listed): entry is TaskEntry => "task" in entry;
/** A task that asks the member whether it is done, after their reply in its thread. */
const asksDone = (entry: Listed): entry is TaskEntry => isTask(entry) && entry.task.asks_done;
const isInvite = (entry: Listed): entry is InviteEntry => "invite" in entry;
const isRow = (entry: Listed): entry is Row => "need" in entry;
const isEvent = (entry: Listed): entry is EventEntry => "event" in entry;
const idOf = (entry: Listed): string => (isEvent(entry) ? entry.id : entry.item.id);

type ActionId = OwnActionId | UniversalActionId;

/** Done's confirmation on a row, by kind ("Done · Marta Ferreira" otherwise). */
const DONE_CONFIRMATION: Partial<Record<RowKind, string>> = {
  highlight: "confirm.seen",
  nudge: "confirm.heardBack",
};

/** Not needed's confirmation on a row, by kind ("Not needed · Marta Ferreira" otherwise). */
const NOT_NEEDED_CONFIRMATION: Partial<Record<RowKind, string>> = {
  warning: "confirm.safe",
  nudge: "confirm.notWaiting",
};

/** Done's label on a row, by kind (Done otherwise). */
const DONE_LABEL: Partial<Record<RowKind, string>> = {
  highlight: "needs.actions.seen",
  nudge: "needs.actions.heardBack",
};

/** What the nudge dialog nudges: a Home row, or a row of Waiting on others. */
interface NudgeTarget {
  conversation: ConversationSummary;
  need: NeedView;
  name: string;
  /** The Home row it came from, which folds away once the nudge is sent. */
  row?: Row | undefined;
}

/** The steps that open a dialog: a pair runs at most one of them. */
const DIALOG_STEPS: ReadonlySet<ActionId> = new Set([
  "reply",
  "remind_when",
  "forward",
  "snooze",
  "nudge",
]);

/** Below 48rem, Waiting on others is a fold after the list rather than in the aside. */
const NARROW = "(width < 48rem)";

function subscribeNarrow(change: () => void): () => void {
  if (typeof globalThis.matchMedia !== "function") {
    return () => {
      // Nothing was listened to.
    };
  }
  const query = globalThis.matchMedia(NARROW);
  query.addEventListener("change", change);
  return () => {
    query.removeEventListener("change", change);
  };
}

const isNarrow = () =>
  typeof globalThis.matchMedia === "function" && globalThis.matchMedia(NARROW).matches;

/** "Ana", for a button ("Nudge Ana…"); an address stays whole. */
const firstNameOf = (name: string) =>
  name.includes("@") ? name : (name.split(/\s+/u, 1)[0] ?? name);

/** What an own action needs to work in an inbox the member may only read. */
const READ_ONLY_STEPS: ReadonlySet<OwnActionId> = new Set(["remind_when", "open_attachment"]);

const kindOf = (need: NeedView): RowKind => {
  // Someone has not written back: whatever its trust, the member is asked
  // nothing but whether to nudge them.
  if (need.reason === "nudge" || need.reason === "waiting") {
    return "nudge";
  }
  if (need.question === "suspicious") {
    return "warning";
  }
  // A rule the member taught keeps it here until they open it: no question.
  if (need.question === "worth_a_look") {
    return "highlight";
  }
  // Scout isn't sure, and the email went to other people: whose move is it?
  if (unsureOf(need) !== undefined) {
    return "unsure";
  }
  // A bill: asked "Is it paid?" until the member answers, then "Pay …".
  const bill = billKind(need);
  if (bill !== undefined) {
    return bill === "ask" ? "bill" : "toPay";
  }
  return need.trust === "check" ? "question" : "need";
};

/**
 * Home: what needs the member, by priority band (Now, Today, This week,
 * Whenever, campbooks-cloud#358), each row saying first what it is and why
 * it is there. The selected item shows its actions under it (its own on
 * 1 to 3, then Done E, Snooze S, Not needed X); ↑/↓ (J/K) move, ←/→ walk
 * the buttons, Enter presses one or opens the item, Esc clears, Z undoes.
 * A row acted on folds away with a one-line confirmation. The rows are the
 * conversations module's records; `attention` only says which, why and by
 * when.
 */
export function WhatNeedsYou({ workspaceId, now, offer, holdSources }: WhatNeedsYouProps) {
  const { t: typedAttention, i18n } = useTranslation("attention");
  const t = typedAttention as unknown as Translate;
  const { t: typedConversations } = useTranslation("conversations");
  const tc = typedConversations as unknown as Translate;
  const { t: typedTasks } = useTranslation("tasks");
  const tt = typedTasks as unknown as Translate;
  const navigate = useNavigate();
  const undo = useUndoToast();
  // Scout's chat, where the app mounted it: Continue with Scout (`.`).
  const scout = useScoutChat();

  const open = useAttention(workspaceId, "open");
  const cleared = useAttention(workspaceId, "cleared");
  // Who Scout waits on for the member (campbooks-cloud#293): never in the
  // queue, listed under Waiting on others until it turns into a nudge.
  const waits = useAttention(workspaceId, "waiting");
  const narrow = useSyncExternalStore(subscribeNarrow, isNarrow, () => false);
  const refresh = useRefreshAttention(workspaceId);
  const snooze = useSnooze(workspaceId);

  // Items acted on, hidden at once while the projection catches up; keyed
  // by their cause, so a follow-up from the sender shows again. A hidden
  // row with a confirmation still shows it while it folds away.
  const [gone, setGone] = useState<ReadonlyMap<string, Entry>>(new Map());
  const openItems = open.data?.items ?? [];

  const clearedItems = cleared.data?.items ?? [];
  // Only waits, whatever else a list holds.
  const waitItems = (waits.data?.items ?? []).filter((item) => item.reason === "waiting");
  // The conversations the items are about, and those the tasks came from.
  const ids = [
    ...new Set(
      [...openItems, ...clearedItems, ...waitItems].flatMap((item) => {
        if (item.record.type === CONVERSATION) {
          return [item.record.id];
        }
        return item.record.type === TASK && item.cause.type === CONVERSATION ? [item.cause.id] : [];
      }),
    ),
  ];
  const conversations = useConversationsByIds(workspaceId, ids);
  const byId = new Map((conversations.data ?? []).map((c) => [c.id, c] as const));
  // Tasks are their own records: read in one batch, as the member sees them.
  const taskIds = openItems
    .filter((item) => item.record.type === TASK)
    .map((item) => item.record.id);
  const tasks = useTasksByIds(workspaceId, taskIds);
  const taskById = new Map((tasks.data ?? []).map((task) => [task.id, task] as const));
  // Invitations awaiting an answer (campbooks-cloud#360): their events, in
  // words, from the schedule module; never also a calendar row below.
  const inviteRefs: InvitationRef[] = openItems
    .filter((item) => item.record.type === INVITATION)
    .map((item) => ({ id: item.record.id, on: item.due_on }));
  const invitations = useInvitations(workspaceId, inviteRefs, { now });
  const invited = new Set(inviteRefs.map((ref) => ref.id));

  // A row still folding away keeps its place even once the list no longer
  // has its item (the server answered and the list refetched first).
  const present = new Set(openItems.map(goneKey));
  const listed = [...openItems];
  for (const row of gone.values()) {
    if (row.confirmation !== undefined && !present.has(goneKey(row.item))) {
      listed.splice(Math.min(row.at ?? listed.length, listed.length), 0, row.item);
    }
  }
  const shown: Entry[] = listed.flatMap((item): Entry[] => {
    const hidden = gone.get(goneKey(item));
    if (hidden !== undefined) {
      return hidden.confirmation === undefined ? [] : [hidden];
    }
    if (item.record.type === TASK) {
      const task = taskById.get(item.record.id);
      if (task?.state !== "open") {
        return [];
      }
      const origin = task.source?.type === CONVERSATION ? byId.get(task.source.id) : undefined;
      return [{ item, task, conversation: origin }];
    }
    if (item.record.type === INVITATION) {
      const invite = invitations.rows.get(item.record.id);
      return invite === undefined ? [] : [{ item, invite, name: invite.who }];
    }
    const conversation = byId.get(item.record.id);
    const need = conversation?.need;
    if (conversation === undefined || need === null || need === undefined) {
      return [];
    }
    const name = awaitedOf(need, conversation)?.name ?? needSender(need, conversation);
    return [{ item, conversation, need, name }];
  });
  const rows = shown.filter((row) => row.confirmation === undefined);
  // By band, Now first (campbooks-cloud#358), in the server's order inside each.
  const groups = groupByBand(shown.map((row) => row.item)).map((group) => ({
    band: group.band,
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
  const [snoozing, setSnoozing] = useState<{ row: Snoozable; remind: boolean } | undefined>();
  // A task's Later dialog (S on a task).
  const [laterFor, setLaterFor] = useState<TaskEntry | undefined>();
  // A Do task's Hold time dialog (H on a task).
  const [holdFor, setHoldFor] = useState<TaskEntry | undefined>();
  // Forward's dialog: whom to send it to.
  const [forwarding, setForwarding] = useState<Row | undefined>();
  // Open attachment: the opened item whose first file takes the focus.
  const [attachmentFor, setAttachmentFor] = useState<string | undefined>();
  // The reply dialog's item, and the draft to pick up again after an undone send.
  const [replying, setReplying] = useState<
    { row: Row; resume?: ReplyResume | undefined; opened: number } | undefined
  >();
  const replies = useRef(0);
  // The nudge dialog's conversation, and the draft to pick up again after an undone send.
  const [nudging, setNudging] = useState<
    { target: NudgeTarget; resume?: NudgeResume | undefined; opened: number } | undefined
  >();
  // Waits the member said they are not waiting on, hidden while the server
  // catches up; keyed by the wait's message, so a later wait shows.
  const [waitsGone, setWaitsGone] = useState<ReadonlySet<string>>(new Set());
  const inboxes = useInboxes(workspaceId).data;
  // The member's calendar (campbooks-cloud#359): Next up above the bands,
  // the rest of today at the end of Today, Coming up in the aside. The
  // schedule module reads it; Home only places it.
  const day = useHomeDay(workspaceId, {
    now,
    selected:
      selection.id?.startsWith(EVENT_PREFIX) === true
        ? selection.id.slice(EVENT_PREFIX.length)
        : undefined,
    hide: invited,
  });
  const asEntry = (event: DayEvent): EventEntry => ({ id: `${EVENT_PREFIX}${event.id}`, event });
  const nextEntry = day.nextUp === undefined ? undefined : asEntry(day.nextUp);
  const todayEvents = day.today.map((event) => asEntry(event));
  // Today shows for its events even when no item is in it.
  const bands =
    todayEvents.length === 0 || groups.some((group) => group.band === "today")
      ? groups
      : [
          ...groups.filter((group) => group.band === "now"),
          { band: "today" as const, rows: [] as Entry[] },
          ...groups.filter((group) => group.band !== "now"),
        ];
  // The order on screen, which the keys follow: Next up, then each band
  // with today's events at the end of Today.
  const nav: Listed[] = [
    ...(nextEntry === undefined ? [] : [nextEntry]),
    ...bands.flatMap((group): Listed[] => [
      ...group.rows.filter((row) => row.confirmation === undefined),
      ...(group.band === "today" ? todayEvents : []),
    ]),
  ];
  const foundAt = nav.findIndex((row) => idOf(row) === selection.id);
  if (nav.length > 0 && (foundAt === -1 || foundAt !== selection.index)) {
    // Adjusting state while rendering, so the change never shows a stale frame.
    const index = foundAt === -1 ? Math.min(selection.index, nav.length - 1) : foundAt;
    const next = nav[index];
    if (next !== undefined && (idOf(next) !== selection.id || index !== selection.index)) {
      setSelection({ id: idOf(next), index });
      setHighlight(undefined);
    }
  }
  const selected = nav.find((row) => idOf(row) === selection.id);
  // The selected need or task (an event is acted on through its own buttons).
  const selectedEntry = selected === undefined || isEvent(selected) ? undefined : selected;

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
  const selectedId = selected === undefined ? undefined : idOf(selected);
  useEffect(() => {
    // The dialog holds the focus while it is open; it comes back here after.
    if (
      !focusFollows.current ||
      selectedId === undefined ||
      highlight !== undefined ||
      snoozing !== undefined ||
      replying !== undefined ||
      forwarding !== undefined ||
      nudging !== undefined
    ) {
      return;
    }
    focusFollows.current = false;
    document.getElementById(itemId(selectedId))?.focus({ preventScroll: true });
  }, [selectedId, highlight, snoozing, replying, forwarding, nudging]);

  const select = (id: string) => {
    setSelection({
      id,
      index: Math.max(
        0,
        nav.findIndex((row) => idOf(row) === id),
      ),
    });
    setHighlight(undefined);
  };

  const hide = (row: Entry, confirmation?: string) => {
    keepFocus();
    const at = openItems.findIndex((item) => goneKey(item) === goneKey(row.item));
    setGone((current) => new Map([...current, [goneKey(row.item), { ...row, confirmation, at }]]));
    const index = nav.findIndex((r) => idOf(r) === row.item.id);
    // The row below takes its place (or the one above, at the end).
    const next = nav[index + 1] ?? nav[index - 1];
    setSelection({
      ...(next === undefined ? {} : { id: idOf(next) }),
      index: nav[index + 1] === undefined ? Math.max(0, index - 1) : index,
    });
    // Reading item by item: the one taking its place opens too.
    setExpandedId((open) => (open === row.item.id && next !== undefined ? idOf(next) : undefined));
    setHighlight(undefined);
  };
  // The confirmation has folded away: the row is simply hidden now.
  const folded = (row: Entry) => {
    setGone((current) => {
      const hidden = current.get(goneKey(row.item));
      return hidden?.confirmation === undefined
        ? current
        : new Map([...current, [goneKey(row.item), { ...hidden, confirmation: undefined }]]);
    });
  };
  const show = (row: Entry) => {
    setGone((current) => new Map([...current].filter(([key]) => key !== goneKey(row.item))));
    select(row.item.id);
  };
  // A hidden row is found among the hidden ones: its summary may already say
  // it needs nothing.
  const rowOf = (conversation: ConversationSummary) =>
    [...rows, ...gone.values()].find(
      (row): row is Row => isRow(row) && row.conversation.id === conversation.id,
    );
  // A task's entry, hidden or shown, when its change is undone or fails.
  const entryOfTask = (task: TaskView) =>
    [...rows, ...gone.values()].find(
      (entry): entry is TaskEntry => isTask(entry) && entry.task.id === task.id,
    );
  // A row of Waiting on others, hidden or shown again with its verdict.
  const showWait = (conversation: ConversationSummary, shown: boolean) => {
    const key = waitKey(conversation);
    setWaitsGone((current) => {
      const next = new Set(current);
      if (shown) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };
  // Nudge: Scout's few lines to the person not written yet, in a dialog over Home.
  const nudge = (target: NudgeTarget, resume?: NudgeResume) => {
    keepFocus();
    setHighlight(undefined);
    replies.current += 1;
    setNudging({ target, resume, opened: replies.current });
  };
  const ahead = useAheadOffer(workspaceId);
  const needActions = useNeedActions(workspaceId, {
    onChanged: refresh,
    onUndo: (conversation) => {
      // Undone (Already paid, Report and delete…): no offer about it.
      ahead.cancel(conversation.id);
      showWait(conversation, true);
      const row = rowOf(conversation);
      if (row !== undefined) {
        show(row);
      }
    },
    onRedo: (conversation) => {
      if (conversation.need?.reason === "waiting") {
        showWait(conversation, false);
      }
      const row = rowOf(conversation);
      if (row !== undefined) {
        hide(row);
      }
    },
    onFailed: (conversation) => {
      showWait(conversation, true);
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
    // Now it waits: the nudge to that person is a dialog away (it is optional).
    // The same answer for the same sender a second time offers the rule that
    // waits on them for the member (campbooks-cloud#378).
    onWaiting: (conversation) => {
      const waiting = conversation.need;
      if (waiting?.reason === "waiting" && (waiting.waiting_on ?? null) !== null) {
        nudge({ conversation, need: waiting, name: displayName(waiting.waiting_on) });
        ahead.offer(conversation, { kind: "waiting" });
      }
    },
    onReported: (conversation) => {
      ahead.offer(conversation, { kind: "report" });
    },
  });

  // Tasks (campbooks-cloud#43): the tasks module does the work and keeps the Undo.
  const taskActions = useTaskActions(workspaceId, {
    now,
    onUndo: (task) => {
      const entry = entryOfTask(task);
      if (entry !== undefined) {
        show(entry);
      }
    },
    onRedo: (task) => {
      const entry = entryOfTask(task);
      if (entry !== undefined) {
        hide(entry);
      }
    },
    onFailed: (task) => {
      const entry = entryOfTask(task);
      if (entry !== undefined) {
        show(entry);
      }
    },
  });

  // Invitations (campbooks-cloud#360): the schedule module answers them and
  // keeps the Undo, for its undo window.
  const entryOfInvite = (eventId: string) =>
    [...rows, ...gone.values()].find(
      (entry): entry is InviteEntry => isInvite(entry) && entry.item.record.id === eventId,
    );
  const invitationAnswers = useInvitationAnswers(workspaceId, {
    onUndo: (eventId) => {
      const entry = entryOfInvite(eventId);
      if (entry !== undefined) {
        show(entry);
      }
    },
    onChanged: refresh,
    onFailed: (eventId) => {
      const entry = entryOfInvite(eventId);
      if (entry !== undefined) {
        show(entry);
      }
    },
  });
  const answerInvite = (entry: InviteEntry, answer: Answer) => {
    const chosen = entry.invite.answers.find((each) => each.id === answer);
    if (chosen === undefined) {
      return;
    }
    hide(entry, chosen.done);
    invitationAnswers.answer(entry.invite.event, answer);
  };

  const when = (until: Date) =>
    new Intl.DateTimeFormat(i18n.language, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit",
    }).format(until);

  const doneTask = (entry: TaskEntry) => {
    const { task } = entry;
    hide(entry, tt(task.verb === "pay" ? "toast.paid" : "toast.done", { title: task.title }));
    taskActions.done(task);
  };
  // Not yet: the task stays open; Home asks again only after a newer reply.
  const notYetTask = (entry: TaskEntry) => {
    hide(entry, tt("toast.stillOpen", { title: entry.task.title }));
    taskActions.notYet(entry.task);
  };
  const notNeededTask = (entry: TaskEntry) => {
    hide(entry, tt("toast.notNeeded", { title: entry.task.title }));
    taskActions.notNeeded(entry.task);
  };
  const done = (entry: Entry) => {
    if (isTask(entry)) {
      doneTask(entry);
      return;
    }
    if (isInvite(entry)) {
      return;
    }
    const row = entry;
    hide(row, t(DONE_CONFIRMATION[kindOf(row.need)] ?? "confirm.done", { name: row.name }));
    needActions.done(row.conversation);
  };
  const notNeeded = (entry: Entry) => {
    if (isTask(entry)) {
      notNeededTask(entry);
      return;
    }
    if (isInvite(entry)) {
      return;
    }
    const row = entry;
    hide(
      row,
      t(NOT_NEEDED_CONFIRMATION[kindOf(row.need)] ?? "confirm.notNeeded", { name: row.name }),
    );
    needActions.notNeeded(row.conversation);
  };
  const confirm = (row: Row) => {
    needActions.confirm(row.conversation);
  };
  // Someone else on the email has the move: the conversation waits on them (the row folds
  // away with Undo), and the nudge to them is offered once the server agreed.
  const waitOn = (row: Row) => {
    const party = unsureOf(row.need)?.party;
    if (party === null || party === undefined) {
      return;
    }
    const name = displayName(party);
    setHighlight(undefined);
    hide(row, t("confirm.waiting", { name }));
    needActions.waitOn(row.conversation, { address: party.address, name });
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
  const snoozeOrRollBack = (row: Snoozable, until?: Date, entry?: number) => {
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
  const snoozeUntil = (row: Snoozable, until: Date) => {
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
  const openConversation = (row: { conversation: ConversationSummary }) => {
    void navigate({
      to: "/w/$workspaceId/inbox/$stream/$conversationId",
      params: {
        workspaceId,
        stream: row.conversation.stream,
        conversationId: row.conversation.id,
      },
    });
  };

  // A row's end: how long it has waited (its due date is in its eyebrow).
  // A nudge says when it was asked ("Asked 14 Oct").
  const rowAge = (row: Row) => {
    const asked = kindOf(row.need) === "nudge";
    const age = ageLabel(row.item.since, now ?? new Date(), i18n.language, { asDate: asked });
    return asked ? { ...age, text: t("nudge.asked", { date: age.text }) } : age;
  };

  // Scout's second line on a question it is unsure of: they haven't answered, whose move is it.
  const unsureNoteOf = (row: Row) => {
    const unsure = unsureOf(row.need);
    return unsure === undefined ? undefined : unsureNote(tc, unsure);
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
    entry: Entry,
  ): {
    own: ItemAction[];
    after: ItemAction[];
    universal: ItemAction[];
    choices: OwnChoice[];
    picked: boolean;
  } => {
    if (isInvite(entry)) {
      // An invitation: its answers on 1 to 3 (none on a calendar that cannot
      // be written), then Snooze, and Open in Calendar on Enter.
      return {
        own: entry.invite.answers.map((answer, index) => ({
          id: answer.id,
          label: answer.label,
          key: CHOICE_KEYS[index] ?? "item.choice1",
        })),
        after: [],
        universal: [
          { id: "snooze", label: t("snooze.button"), key: UNIVERSAL_KEYS.snooze },
          { id: "calendar", label: t("calendar.openInCalendar"), key: "list.open" },
        ],
        choices: [],
        picked: false,
      };
    }
    if (asksDone(entry)) {
      // "You replied. Is this done?": Done and Not yet on 1 and 2, then the
      // thread and Scout; the rest of a task's actions wait for the answer.
      const chat = entry.conversation;
      return {
        own: [
          { id: "done", label: tt("detail.done"), key: CHOICE_KEYS[0] },
          { id: NOT_YET, label: tt("detail.notYet"), key: CHOICE_KEYS[1] },
        ],
        after: [
          ...(chat === undefined
            ? []
            : [{ id: THREAD, label: tt("ask.openThread"), key: THREAD_KEY }]),
          ...(chat === undefined || scout === undefined
            ? []
            : [{ id: SCOUT, label: t("actions.scout"), ai: true, key: SCOUT_KEY }]),
        ],
        universal: [],
        choices: [],
        picked: false,
      };
    }
    if (isTask(entry)) {
      // A task has no questions: Done (Paid), Later, Hold time (a Do task), Not needed.
      return {
        own: [],
        after: [],
        universal: [
          {
            id: "done",
            label: tt(entry.task.verb === "pay" ? "detail.paid" : "detail.done"),
            key: UNIVERSAL_KEYS.done,
          },
          { id: "snooze", label: tt("detail.later"), key: UNIVERSAL_KEYS.snooze },
          ...(holdSources !== undefined && holdable(entry.task)
            ? [{ id: HOLD, label: tt("detail.holdTime"), key: HOLD_KEY }]
            : []),
          { id: "not_needed", label: tt("detail.notNeeded"), key: UNIVERSAL_KEYS.not_needed },
        ],
        choices: [],
        picked: false,
      };
    }
    const row = entry;
    const kind = kindOf(row.need);
    const unsure = unsureOf(row.need);
    const ranked = rowChoices(kind, row.need.actions);
    // Nudge needs someone it is natural to nudge: with nobody obvious, it is not offered.
    const all =
      kind === "unsure" && (unsure?.party ?? null) === null
        ? { ...ranked, own: ranked.own.filter((choice) => choice.id !== "wait") }
        : ranked;
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
        case "nudge": {
          return t("actions.nudge", { name: firstNameOf(row.name) });
        }
        // Scout isn't sure: the person it went to, the person who wrote, or nothing.
        case "wait": {
          // The whole name: the person it went to may be an organisation ("Águas do Ribatejo").
          return t("actions.nudge", { name: displayName(unsure?.party) });
        }
        case "reply_to": {
          return tc("needs.actions.replyTo", { name: firstNameOf(row.name) });
        }
        case "nothing": {
          return tc("needs.actions.nothingForMe");
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
      done: tc(DONE_LABEL[kind] ?? "needs.actions.done"),
      snooze: t("snooze.button"),
      not_needed: tc(kind === "nudge" ? "needs.actions.notWaiting" : "needs.actions.notNeeded"),
    };
    const own = choices.slice(0, CHOICE_KEYS.length);
    return {
      own: own.map((choice, index) => ({
        id: choice.id,
        label: choiceLabel(choice),
        // Reply asks Scout for likely replies, Nudge for the nudge: the visor marks them.
        ai: choice.steps.includes("reply") || choice.steps.includes("nudge"),
        key: CHOICE_KEYS[index] ?? "item.choice1",
      })),
      // After its own actions, in the same style, never on 1 to 3.
      after:
        scout === undefined
          ? []
          : [{ id: SCOUT, label: t("actions.scout"), ai: true, key: SCOUT_KEY }],
      universal: universal.map((id) => ({
        id,
        label: universalLabel[id],
        key: UNIVERSAL_KEYS[id],
      })),
      choices: own,
      picked: all.picked && own.length > 0,
    };
  };
  // An event's row (campbooks-cloud#359): Join (its web place) and Open
  // email (the email it was read from) on 1 and 2, then Open in Calendar on
  // Enter.
  const eventActionsOf = (entry: EventEntry): ReturnType<typeof actionsOf> => {
    const { event } = entry;
    const choices = [
      ...(event.join === undefined ? [] : [{ id: "join", label: t("calendar.join") }]),
      ...(event.openEmail === undefined ? [] : [{ id: "email", label: t("calendar.openEmail") }]),
    ];
    return {
      own: choices.map((choice, index) => ({
        ...choice,
        key: CHOICE_KEYS[index] ?? "item.choice1",
      })),
      after: [],
      universal: [{ id: "calendar", label: t("calendar.openInCalendar"), key: "list.open" }],
      choices: [],
      picked: false,
    };
  };
  const runEvent = ({ event }: EventEntry, id: string) => {
    switch (id) {
      case "join": {
        event.join?.();
        return;
      }
      case "email": {
        event.openEmail?.();
        return;
      }
      case "calendar": {
        event.openInCalendar();
        return;
      }
      default: {
        return;
      }
    }
  };
  const runInvite = (entry: InviteEntry, id: string) => {
    switch (id) {
      case "snooze": {
        startSnooze(entry);
        return;
      }
      case "calendar": {
        entry.invite.openInCalendar();
        return;
      }
      default: {
        const answer = entry.invite.answers.find((each) => each.id === id);
        if (answer !== undefined) {
          answerInvite(entry, answer.id);
        }
      }
    }
  };
  const startSnooze = (entry: Entry, remind = false) => {
    keepFocus();
    setHighlight(undefined);
    if (isTask(entry)) {
      setLaterFor(entry);
      return;
    }
    setSnoozing({ row: entry, remind });
  };
  const startHold = (entry: TaskEntry) => {
    keepFocus();
    setHighlight(undefined);
    setHoldFor(entry);
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
      case "nudge": {
        nudge({ conversation: row.conversation, need: row.need, name: row.name, row });
        return;
      }
      case "wait": {
        waitOn(row);
        return;
      }
      case "reply_to": {
        // Yes first, so the reply is a correction of what Scout was unsure of; then the dialog.
        setHighlight(undefined);
        confirm(row);
        reply(row);
        return;
      }
      case "nothing": {
        notNeeded(row);
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
  const run = (entry: Listed, id: string) => {
    if (isEvent(entry)) {
      runEvent(entry, id);
      return;
    }
    if (isInvite(entry)) {
      runInvite(entry, id);
      return;
    }
    if (isTask(entry)) {
      switch (id) {
        case "done": {
          doneTask(entry);
          return;
        }
        case NOT_YET: {
          notYetTask(entry);
          return;
        }
        case THREAD: {
          if (entry.conversation !== undefined) {
            openConversation({ conversation: entry.conversation });
          }
          return;
        }
        case SCOUT: {
          continueWithScout(entry);
          return;
        }
        case "snooze": {
          startSnooze(entry);
          return;
        }
        case HOLD: {
          startHold(entry);
          return;
        }
        case "not_needed": {
          notNeededTask(entry);
          return;
        }
        default: {
          return;
        }
      }
    }
    const row = entry;
    if (id === SCOUT) {
      continueWithScout(row);
      return;
    }
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
  // Continue with Scout: the panel opens on the need's conversation; the
  // need stays selected, and the focus comes back to the button on close.
  const continueWithScout = (row: { conversation?: ConversationSummary | undefined }) => {
    // Only the selected row shows its actions: its button is the one.
    const button =
      list.current?.querySelector<HTMLElement>(`[data-action="${CSS.escape(SCOUT)}"]`) ?? null;
    if (row.conversation !== undefined) {
      scout?.(conversationRef(row.conversation.id), { from: button });
    }
  };
  let selectedActions: ReturnType<typeof actionsOf> | undefined;
  if (selected !== undefined) {
    selectedActions = isEvent(selected) ? eventActionsOf(selected) : actionsOf(selected);
  }
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
    const index = nav.findIndex((row) => idOf(row) === selectedId);
    const target = Math.min(nav.length - 1, Math.max(0, index + step));
    const next = nav[target];
    if (next !== undefined) {
      setSelection({ id: idOf(next), index: target });
      setHighlight(undefined);
      // The row stepped onto opens, as a click on it does.
      setExpandedId(idOf(next));
      const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
        document.getElementById(itemId(idOf(next)));
      // Not every environment scrolls (tests run without layout).
      element?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const walk = (step: 1 | -1) => {
    if (selectedActions === undefined) {
      return;
    }
    const all = [...selectedActions.own, ...selectedActions.after, ...selectedActions.universal];
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
      if (highlight !== undefined) {
        run(selected, highlight);
        return;
      }
      // An event, or an invitation, opens in Calendar, on its day.
      if (isEvent(selected)) {
        selected.event.openInCalendar();
        return;
      }
      if (isInvite(selected)) {
        selected.invite.openInCalendar();
        return;
      }
      setExpandedId((id) => (id === selected.item.id ? undefined : selected.item.id));
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
      if (selectedEntry !== undefined) {
        done(selectedEntry);
      }
    },
    { enabled: offered("done") },
  );
  useShortcut(
    "item.snooze",
    () => {
      if (selectedEntry !== undefined) {
        startSnooze(selectedEntry);
      }
    },
    { enabled: offered("snooze") },
  );
  // H: a Do task's Hold time.
  useShortcut(
    HOLD_KEY,
    () => {
      if (selected !== undefined && isTask(selected)) {
        startHold(selected);
      }
    },
    { enabled: selectedActions?.universal.some((action) => action.id === HOLD) === true },
  );
  // X: not needed, a warning's "It's safe" or a question's No.
  useShortcut(
    "conversations.resolve_need",
    () => {
      if (selectedEntry !== undefined) {
        notNeeded(selectedEntry);
      }
    },
    { enabled: offered("not_needed") || offered("no") || offered("safe") },
  );
  // Shift+E: every bill the Pay group asks about is already paid.
  const askedBills = order.filter((row): row is Row => isRow(row) && kindOf(row.need) === "bill");
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
      if (selectedEntry !== undefined && isRow(selectedEntry)) {
        reply(selectedEntry);
      }
    },
    { enabled: offered("reply") },
  );
  useShortcut(
    SCOUT_KEY,
    () => {
      if (selectedEntry !== undefined && (isRow(selectedEntry) || asksDone(selectedEntry))) {
        continueWithScout(selectedEntry);
      }
    },
    { enabled: hasRows && scout !== undefined },
  );
  // O: the thread a task asking "Is it done?" came from.
  useShortcut(
    THREAD_KEY,
    () => {
      if (selected !== undefined && asksDone(selected) && selected.conversation !== undefined) {
        openConversation({ conversation: selected.conversation });
      }
    },
    { enabled: selectedActions?.after.some((action) => action.id === THREAD) === true },
  );

  const today = now ?? new Date();
  // A row's eyebrow (campbooks-cloud#358): its kind, then why it is in its
  // band, naming the person by their first name.
  const eyebrowOf = (entry: Entry): ItemEyebrow => {
    const { item } = entry;
    const name = isTask(entry) ? undefined : firstNameOf(entry.name);
    const reason = reasonSentence(item.band_reason, { name, today, locale: i18n.language });
    let kind = `kinds.${item.reason}`;
    if (isTask(entry)) {
      kind = entry.task.verb === "pay" ? "kinds.payTask" : "kinds.doTask";
    } else if (isRow(entry) && kindOf(entry.need) === "unsure") {
      kind = "kinds.unsure";
    }
    return {
      signal: signalOf(item.reason),
      kind: t(kind),
      reason: t(reason.key, reason.values),
      danger: isPressing(item.band_reason),
    };
  };
  // Why it's here, in the opened item: the band and why, then what the band
  // holds and, for a bill or a nudge, what Campbooks does about it.
  const whyOf = (row: Row) => {
    const band = bandOf(row.item);
    const sentence = reasonSentence(row.item.band_reason, {
      name: firstNameOf(row.name),
      today,
      locale: i18n.language,
    });
    const reason = t(sentence.key, sentence.values);
    // A sentence starting with the person's name keeps its capital.
    const named = "name" in sentence.values && reason.startsWith(String(sentence.values["name"]));
    const notes = [t(`bandNotes.${band}`)];
    const kind = kindOf(row.need);
    if (kind === "bill") {
      notes.push(t("pay.note"));
    }
    if (kind === "nudge") {
      notes.push(t("nudge.note"));
      const nudges = row.need.nudges ?? 0;
      if (nudges > 0) {
        notes.push(t("nudge.nudged", { count: nudges }));
      }
    }
    return {
      label: t("why.title"),
      text: t("why.sentence", {
        band: t(`bands.${band}`),
        reason: named
          ? reason
          : reason.charAt(0).toLocaleLowerCase(i18n.language) + reason.slice(1),
      }),
      notes,
    };
  };
  const dateLine = new Intl.DateTimeFormat(i18n.language, {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(today);
  const loading =
    open.isPending ||
    (ids.length > 0 && conversations.isPending) ||
    (taskIds.length > 0 && tasks.isPending) ||
    invitations.loading;
  // "Worth a look" asks nothing of the member: it has its own group and
  // count, and stays out of "N things need you" (campbooks-cloud#282).
  const count = order.filter((row) => row.item.reason !== "worth_a_look").length;
  const looks = order.length - count;
  // The day's facts when the calendar has some (campbooks-cloud#359): the
  // next event and how many more today, in place of the general line.
  const facts =
    day.facts === undefined
      ? undefined
      : [
          t("calendar.next", { title: day.facts.title, time: day.facts.time }),
          ...(day.facts.more > 0 ? [t("calendar.more", { count: day.facts.more })] : []),
        ].join(" ");
  // Nothing asks anything of them: say so, and what is only worth a look.
  let subline = facts ?? t("hero.subline");
  if (count === 0 && !loading) {
    subline =
      looks > 0 ? t("hero.lookSubline", { count: looks }) : (facts ?? t("hero.clearSubline"));
  }
  const review = order.some((row) => row.item.reason === "review");
  const times = snoozeOptions(today);
  // Remind's times: a bill's reminder before its due date first, then Snooze's.
  const billDue = (row: Snoozable): row is Row =>
    isRow(row) && ["bill", "toPay"].includes(kindOf(row.need));
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

  // Who Scout waits on, soonest check first (the server's order), less the
  // ones the member just said they are not waiting on.
  const waiting = waitItems.flatMap((item) => {
    const conversation = byId.get(item.record.id);
    const need = conversation?.need;
    if (
      conversation === undefined ||
      need?.reason !== "waiting" ||
      waitsGone.has(waitKey(conversation))
    ) {
      return [];
    }
    const name = awaitedOf(need, conversation)?.name ?? needSender(need, conversation);
    const row: WaitingRow = {
      id: item.id,
      name,
      subject: conversation.subject === "" ? t("noSubject") : conversation.subject,
      checks: checksLabel(item.due_on, today, i18n.language, t),
      checksOn: checksDate(item.due_on, i18n.language),
      nudges: need.nudges ?? 0,
    };
    return [{ row, target: { conversation, need, name } }];
  });
  const waitingList =
    waiting.length === 0 ? null : (
      <WaitingOnOthers
        rows={waiting.map((wait) => wait.row)}
        t={t}
        onNudge={(row) => {
          const wait = waiting.find((each) => each.row.id === row.id);
          if (wait !== undefined) {
            nudge(wait.target);
          }
        }}
        onNotWaiting={(row) => {
          const wait = waiting.find((each) => each.row.id === row.id);
          if (wait !== undefined) {
            showWait(wait.target.conversation, false);
            needActions.notNeeded(wait.target.conversation);
          }
        }}
        onAsk={
          scout === undefined
            ? undefined
            : (row, from) => {
                const wait = waiting.find((each) => each.row.id === row.id);
                if (wait !== undefined) {
                  scout(conversationRef(wait.target.conversation.id), { from });
                }
              }
        }
      />
    );

  // A Coming up line's end: "Add to my calendar" for what only the member
  // sees, else its time.
  const comingEnd = (line: DayLine): ReactNode => {
    if (line.add !== undefined) {
      return (
        <Button variant="link" aria-label={line.add.name} onClick={line.add.run}>
          {line.add.label}
        </Button>
      );
    }
    return line.meta === undefined ? undefined : <LineMeta>{line.meta}</LineMeta>;
  };
  // Coming up (campbooks-cloud#359): the week after today, soonest first;
  // "Add to my calendar" on what only the member sees.
  const coming =
    day.coming.length === 0 ? null : (
      <AsideSection title={t("calendar.comingTitle")}>
        <LineMeta>{t("calendar.comingSub")}</LineMeta>
        {day.coming.map((line) => (
          <Line
            key={line.id}
            time={line.day}
            leading={
              <KindDot kind={line.kind} hollow={line.onlyYou !== undefined} label={line.onlyYou} />
            }
            trailing={comingEnd(line)}
          >
            {line.title}
          </Line>
        ))}
        <div className="flex">
          <ButtonLink
            variant="link"
            shortcut="nav.schedule"
            href={day.calendarHref}
            onClick={(event) => {
              event.preventDefault();
              day.openCalendar();
            }}
          >
            {t("calendar.openCalendar")}
          </ButtonLink>
        </div>
      </AsideSection>
    );

  const aside = (
    <>
      {coming}
      {narrow || waitingList === null ? null : (
        <AsideSection title={t("waiting.title")}>
          <LineMeta>{t("waiting.sub")}</LineMeta>
          {waitingList}
        </AsideSection>
      )}
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
    </>
  );

  // A task that asks "You replied. Is this done?" (campbooks-cloud#396): the
  // reply and when, what it is, Scout's question; selected, Done and Not yet.
  // Opened, why Scout asks: it saw that the member replied, never what they
  // wrote.
  const renderAskDone = (entry: TaskEntry, repliedAt: string, from: string | undefined) => {
    const { task } = entry;
    const isSelected = entry.item.id === selectedId;
    const expanded = expandedId === entry.item.id;
    const actions = isSelected ? actionsOf(entry) : undefined;
    const clock = now ?? new Date();
    const time = replyWhen(repliedAt, i18n.language, clock);
    const name = from ?? tt("detail.someone");
    return (
      <Item
        key={entry.item.id}
        id={itemId(entry.item.id)}
        eyebrow={{
          signal: "do",
          kind: tt("ask.replied"),
          reason: ageLabel(repliedAt, clock, i18n.language).text,
        }}
        who={tt("ask.meta", { kind: tt("add.do"), name })}
        what={task.title}
        note={tt("ask.line", { name })}
        avatar={{ name, kind: "person" }}
        selected={isSelected}
        expanded={expanded}
        confirmation={entry.confirmation}
        onConfirmEnd={() => {
          folded(entry);
        }}
        onToggle={() => {
          select(entry.item.id);
          setExpandedId(expanded ? undefined : entry.item.id);
        }}
        actions={
          actions === undefined ? undefined : (
            <ItemActions
              label={t("actions.label", { name: task.title })}
              own={actions.own}
              after={actions.after}
              universal={actions.universal}
              highlighted={highlight}
              onHighlight={setHighlight}
              onAction={(id) => {
                run(entry, id);
              }}
            />
          )
        }
        detail={
          <ItemDetail
            why={{
              label: tt("ask.whyTitle"),
              text: tt("ask.whyText", { time }),
              notes: [tt("ask.whyNote")],
            }}
            actions={
              <Button
                variant="link"
                onClick={() => {
                  void navigate({ to: `/w/${workspaceId}/tasks/${task.id}` });
                }}
              >
                {tt("ask.openTask")}
              </Button>
            }
          />
        }
      />
    );
  };

  // A task as a row: its eyebrow (its kind and why it is here, its date
  // among them), where it came from, its title; the row opens it in Tasks.
  const renderTask = (entry: TaskEntry) => {
    const { task, conversation } = entry;
    const isSelected = entry.item.id === selectedId;
    const actions = isSelected ? actionsOf(entry) : undefined;
    const held = heldText(task, tt, i18n.language);
    const from = conversation === undefined ? undefined : displayName(conversation.latest_from);
    if (asksDone(entry) && task.replied !== null && task.replied !== undefined) {
      return renderAskDone(entry, task.replied.at, from);
    }
    return (
      <Item
        key={entry.item.id}
        id={itemId(entry.item.id)}
        eyebrow={eyebrowOf(entry)}
        who={from === undefined ? tt("row.own") : tt("row.fromEmail", { name: from })}
        what={task.title}
        avatar={{ name: from ?? task.title, kind: task.verb === "pay" ? "service" : "person" }}
        pills={held === undefined ? undefined : [{ text: held, signal: "do" }]}
        selected={isSelected}
        confirmation={entry.confirmation}
        onConfirmEnd={() => {
          folded(entry);
        }}
        onToggle={() => {
          select(entry.item.id);
          void navigate({ to: `/w/${workspaceId}/tasks/${task.id}` });
        }}
        actions={
          actions === undefined ? undefined : (
            <ItemActions
              label={t("actions.label", { name: task.title })}
              own={actions.own}
              after={actions.after}
              universal={actions.universal}
              highlighted={highlight}
              onHighlight={setHighlight}
              onAction={(id) => {
                run(entry, id);
              }}
            />
          )
        }
      />
    );
  };

  // An invitation as a row (campbooks-cloud#360): Decide and the day of the
  // event, who asked, "Invitation: <title>", when and where, and whether the
  // member is free then; selected, its answers.
  const renderInvite = (entry: InviteEntry) => {
    const { invite } = entry;
    const isSelected = entry.item.id === selectedId;
    const actions = isSelected ? actionsOf(entry) : undefined;
    return (
      <Item
        key={entry.item.id}
        id={itemId(entry.item.id)}
        eyebrow={eyebrowOf(entry)}
        who={entry.name}
        what={invite.what}
        due={invite.when}
        avatar={{ name: entry.name, kind: invite.person ? "person" : "service" }}
        tags={invite.tags}
        age={ageLabel(entry.item.since, now ?? new Date(), i18n.language)}
        selected={isSelected}
        confirmation={entry.confirmation}
        onConfirmEnd={() => {
          folded(entry);
        }}
        onToggle={() => {
          select(entry.item.id);
        }}
        actions={
          actions === undefined ? undefined : (
            <ItemActions
              label={t("actions.label", { name: invite.title })}
              own={actions.own}
              universal={actions.universal}
              highlighted={highlight}
              onHighlight={setHighlight}
              onAction={(id) => {
                runInvite(entry, id);
              }}
            />
          )
        }
      />
    );
  };

  // An event as a row (campbooks-cloud#359): its kind's square, the kind and
  // when, its title and where, its time at the end; selected, its actions.
  const renderEvent = (entry: EventEntry) => {
    const { event } = entry;
    const isSelected = entry.id === selectedId;
    const actions = isSelected ? eventActionsOf(entry) : undefined;
    return (
      <Item
        key={entry.id}
        id={itemId(entry.id)}
        kind={event.kind}
        eyebrow={event.eyebrow}
        who={event.title}
        what={event.meta}
        due={event.when}
        selected={isSelected}
        onToggle={() => {
          select(entry.id);
        }}
        actions={
          actions === undefined ? undefined : (
            <ItemActions
              label={t("actions.label", { name: event.title })}
              own={actions.own}
              universal={actions.universal}
              highlighted={highlight}
              onHighlight={setHighlight}
              onAction={(id) => {
                runEvent(entry, id);
              }}
            />
          )
        }
      />
    );
  };

  // At the end of the list: every bill Campbooks asks about, paid at once;
  // and the tasks that are not for today (campbooks-cloud#358).
  const elsewhere = open.data?.tasks_elsewhere ?? 0;
  const ends =
    askedBills.length > 1 || elsewhere > 0 ? (
      <div className="flex flex-col gap-1">
        {askedBills.length > 1 ? (
          <Line
            trailing={
              <Button
                shortcut="item.all_paid"
                onClick={() => {
                  allPaid(askedBills);
                }}
              >
                {t("pay.allPaid", { count: askedBills.length })}
              </Button>
            }
          >
            {t("pay.asked", { count: askedBills.length })}
          </Line>
        ) : null}
        {elsewhere > 0 ? (
          <Line
            trailing={
              <ButtonLink
                variant="link"
                shortcut="nav.tasks"
                href={`/w/${workspaceId}/tasks`}
                onClick={(event) => {
                  event.preventDefault();
                  void navigate({ to: `/w/${workspaceId}/tasks` });
                }}
              >
                {t("openTasks")}
              </ButtonLink>
            }
          >
            {t("tasksElsewhere", { count: elsewhere })}
          </Line>
        ) : null}
      </div>
    ) : null;

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
        subline={[
          subline,
          ...(waiting.length > 0 ? [t("hero.waiting", { count: waiting.length })] : []),
        ].join(" ")}
      />
      {offer}
      {loading ? <SkeletonList rows={4} /> : null}
      {/* Groups, Scout noticed and an opened item each keep a section's room. */}
      <div ref={list} className="flex flex-col gap-6">
        {nextEntry === undefined ? null : (
          <QueueGroup name={t("calendar.nextUp")}>{renderEvent(nextEntry)}</QueueGroup>
        )}
        {bands.map((group, position) => {
          const live = group.rows.filter((row) => row.confirmation === undefined);
          // Today's events close Today, after its items; they are not counted.
          const events =
            group.band === "today" ? todayEvents.map((entry) => renderEvent(entry)) : [];
          const items = group.rows.map((row) => {
            if (isTask(row)) {
              return renderTask(row);
            }
            if (isInvite(row)) {
              return renderInvite(row);
            }
            const expanded = expandedId === row.item.id;
            const isSelected = row.item.id === selectedId;
            const actions = isSelected ? actionsOf(row) : undefined;
            return (
              <Item
                key={row.item.id}
                id={itemId(row.item.id)}
                eyebrow={eyebrowOf(row)}
                who={row.name}
                what={needSentence(
                  tc,
                  row.need,
                  row.name,
                  { locale: i18n.language, now },
                  row.conversation,
                )}
                excerpt={row.need.quote?.text}
                note={unsureNoteOf(row)}
                avatar={{
                  name: row.name,
                  kind:
                    row.item.reason === "pay" || row.need.question === "suspicious"
                      ? "service"
                      : "person",
                }}
                age={rowAge(row)}
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
                      after={actions.after}
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
                    why={whyOf(row)}
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
          });
          // The last row folding away takes its group with it: no "Today 0".
          // A Today of events only is headed by its name alone.
          return (
            <Fragment key={group.band}>
              {live.length === 0 && events.length === 0 ? (
                <div className="flex flex-col">{items}</div>
              ) : (
                <QueueGroup
                  tone={group.band === "now" ? "danger" : undefined}
                  name={t(`bands.${group.band}`)}
                  count={live.length === 0 ? undefined : live.length}
                >
                  {items}
                  {events}
                </QueueGroup>
              )}
              {position === 0 ? noticed : null}
            </Fragment>
          );
        })}
        {bands.length === 0 ? noticed : null}
        {ends}
      </div>
      {narrow && waitingList !== null ? (
        <Disclosure summary={t("waiting.fold", { count: waiting.length })} defaultOpen>
          {waitingList}
        </Disclosure>
      ) : null}
      {/* Not under the nudge dialog: its keys (1, Esc) are the dialog's while it is open. */}
      {nudging === undefined ? ahead.element : null}
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
      <Dialog
        open={laterFor !== undefined}
        onClose={() => {
          setLaterFor(undefined);
        }}
        title={tt("laterDialog.title")}
        description={tt("laterDialog.description")}
      >
        <DialogChoices
          label={tt("laterDialog.choices")}
          choices={times.map((option) => ({
            id: option.key,
            label: t(`snooze.${option.key}`),
            detail: when(option.until),
          }))}
          onChoose={(key) => {
            const entry = laterFor;
            const option = times.find((o) => o.key === key);
            setLaterFor(undefined);
            if (entry !== undefined && option !== undefined) {
              hide(entry, tt("toast.later", { title: entry.task.title, when: when(option.until) }));
              taskActions.later(entry.task, option.until);
            }
          }}
        />
      </Dialog>
      <Dialog
        open={holdFor !== undefined}
        onClose={() => {
          setHoldFor(undefined);
        }}
        title={tt("holdDialog.title")}
        description={tt("holdDialog.description")}
      >
        {holdFor === undefined || holdSources === undefined ? null : (
          <HoldTime
            workspaceId={workspaceId}
            task={holdFor.task}
            sources={holdSources}
            now={now}
            onPick={(pick) => {
              const entry = holdFor;
              setHoldFor(undefined);
              taskActions.hold(entry.task, pick);
            }}
          />
        )}
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
      {nudging === undefined ? null : (
        <NudgeDialog
          key={nudging.opened}
          workspaceId={workspaceId}
          conversationId={nudging.target.conversation.id}
          need={nudging.target.need}
          resume={nudging.resume}
          onClose={() => {
            setNudging(undefined);
          }}
          onOpenDraft={() => {
            // The nudge waits in the reader, addressed, to finish there.
            setNudging(undefined);
            openConversation(nudging.target);
          }}
          onSent={() => {
            const { row, name } = nudging.target;
            if (row !== undefined) {
              hide(row, t("confirm.nudged", { name: firstNameOf(name) }));
            }
          }}
          onUndone={(resume) => {
            if (nudging.target.row !== undefined) {
              show(nudging.target.row);
            }
            nudge(nudging.target, resume);
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

/** A calendar date ("2026-10-19") at UTC midnight. */
function utcDate(isoDate: string): Date {
  const [year, month, day] = isoDate.split("-").map(Number);
  return new Date(Date.UTC(year ?? 0, (month ?? 1) - 1, day ?? 1));
}

/** When Scout checks a wait again: "Checks Mon" within the week, else "Checks 30 Oct". */
function checksLabel(
  dueOn: string | null | undefined,
  today: Date,
  locale: string,
  t: Translate,
): string {
  if (dueOn === null || dueOn === undefined) {
    return "";
  }
  const date = utcDate(dueOn);
  const days = Math.round(
    (date.getTime() - Date.UTC(today.getFullYear(), today.getMonth(), today.getDate())) /
      86_400_000,
  );
  const day = new Intl.DateTimeFormat(locale, {
    timeZone: "UTC",
    ...(days >= 0 && days < 7 ? { weekday: "short" } : { day: "numeric", month: "short" }),
  }).format(date);
  return t("waiting.checks", { day });
}

/** The date a wait is checked again, whole, for the tooltip. */
function checksDate(
  dueOn: string | null | undefined,
  locale: string,
): { dateTime: string; label: string } | undefined {
  if (dueOn === null || dueOn === undefined) {
    return undefined;
  }
  return {
    dateTime: dueOn,
    label: new Intl.DateTimeFormat(locale, { timeZone: "UTC", dateStyle: "full" }).format(
      utcDate(dueOn),
    ),
  };
}

const ITEM_ID_PREFIX = "attention-";
const itemId = (id: string) => `${ITEM_ID_PREFIX}${id}`;

const goneKey = (item: ItemView) => `${item.id}:${item.cause.id}`;

const waitKey = (conversation: ConversationSummary) =>
  `${conversation.id}:${conversation.need?.message_id ?? ""}`;

interface NeedDetailProps {
  workspaceId: string;
  row: Row;
  /** Why it's here (campbooks-cloud#358). */
  why: { label: string; text: string; notes: string[] };
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

/**
 * An opened item: the warning's tells, Scout's summary, why it is here, the
 * email itself and how it was decided.
 */
function NeedDetail({
  workspaceId,
  row,
  why,
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
  const unsure = unsureOf(need);
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
        why={why}
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
        quoteFrom={quoteAttribution(need, needSender(need, row.conversation), locale)}
        quote={need.quote?.text}
        actions={
          <Button variant="link" onClick={onOpen}>
            {t("actions.open")}
          </Button>
        }
        how={
          <HowDecided
            rows={howRows(
              tc,
              exists,
              decisions,
              unsure === undefined ? undefined : { unsure, question: need.question },
            )}
            footnote={tc("needs.how.footnote")}
          />
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

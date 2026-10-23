import { useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Button,
  EmptyState,
  Heading,
  ListPane,
  PaneHeader,
  QueueGroup,
  ReaderPane,
  SkeletonList,
  Spacer,
  TaskRow,
  Text,
  type TaskRowChip,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { useHandedOff, useTasks, type TaskView } from "../../api/queries";
import { dueThisWeek, GROUPS, groupTasks, listOrder, type Group } from "../../model";
import { CONVERSATION, type TaskSource, type UseTaskSources } from "../../source";
import { dueText, planText } from "../../task-text";
import { QuickAdd } from "../QuickAdd";
import { TaskDetail } from "../TaskDetail";
import { memberName } from "../HandOff";
import { useTaskActions } from "../use-task-actions";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

/** Which signal colour marks each group's dot. */
const SIGNAL = {
  overdue: "review",
  today: "do",
  coming: "decide",
  none: "sort",
  later: "look",
} as const satisfies Record<Group, string>;

export interface TasksPageProps {
  workspaceId: string;
  /** The task open beside the list. */
  taskId?: string | undefined;
  /** Changes whenever `n t` asks to add a task from another page. */
  addRequest?: number | undefined;
  /** Where tasks made from conversations came from. */
  useSources: UseTaskSources;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
}

const ROW_PREFIX = "task-";
const rowId = (id: string) => `${ROW_PREFIX}${id}`;

/** Newest decision of the list: overdue first, then by due day and time, then undated. */
function byDue(a: TaskView, b: TaskView): number {
  const key = (task: TaskView) => [task.due_on ?? "9999-99-99", task.due_at ?? ""] as const;
  const [dayA, timeA] = key(a);
  const [dayB, timeB] = key(b);
  return (
    dayA.localeCompare(dayB) ||
    timeA.localeCompare(timeB) ||
    a.created_at.localeCompare(b.created_at)
  );
}

/**
 * Tasks: the member's open tasks in Overdue, Today, Coming up and No date
 * yet, and a folded Later; quick add above them; a task's detail beside the
 * list from `lg`, replacing it below. Keys: `n t` adds, J/K move, Enter
 * opens, E marks done (paid), S offers Later, X marks not needed, Esc closes
 * the detail, Z undoes.
 */
export function TasksPage({ workspaceId, taskId, addRequest, useSources, now }: TasksPageProps) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as Translate;
  const navigate = useNavigate();
  const members = useMembers(workspaceId).data ?? [];
  const me = members.find((member) => member.you)?.user_id;
  const mine = useTasks(workspaceId, me);
  const away = useHandedOff(workspaceId, me);
  const actions = useTaskActions(workspaceId, { now });
  const clock = now ?? new Date();

  const input = useRef<HTMLInputElement>(null);
  const [laterOpen, setLaterOpen] = useState(false);
  const [offerLater, setOfferLater] = useState(false);
  const [cursor, setCursor] = useState<string | undefined>();

  const own = mine.data ?? [];
  const handed = away.data ?? [];
  const all = [...own, ...handed].toSorted(byDue);
  const groups = groupTasks(all, clock);
  const order = listOrder(groups, laterOpen);
  const open = taskId === undefined ? undefined : all.find((task) => task.id === taskId);
  const selected = order.find((task) => task.id === cursor) ?? open ?? order[0];
  // E, X and S act on the task that is open (its buttons carry the key hints);
  // with none open, on the one the cursor is on.
  const subject = open ?? selected;

  // Sources of the tasks that came from a conversation.
  const sourceIds = [
    ...new Set(all.flatMap((task) => (task.source?.type === CONVERSATION ? [task.source.id] : []))),
  ];
  const sources = useSources(workspaceId, sourceIds);
  const sourceOf = (task: TaskView): TaskSource | undefined =>
    task.source?.type === CONVERSATION ? sources.get(task.source.id) : undefined;
  const nameOf = (id: string | null | undefined) => {
    const member = members.find((candidate) => candidate.user_id === id);
    return member === undefined ? t("detail.someone") : memberName(member);
  };

  const goTo = (path: string) => {
    void navigate({ to: path });
  };
  const base = `/w/${workspaceId}/tasks`;
  const openTask = (task: TaskView) => {
    setCursor(task.id);
    setOfferLater(false);
    goTo(`${base}/${task.id}`);
  };
  const closeDetail = () => {
    setOfferLater(false);
    goTo(base);
  };

  // `n t` from another page: the field is focused once the page is here.
  useEffect(() => {
    if (addRequest !== undefined) {
      input.current?.focus();
    }
  }, [addRequest]);
  useShortcut("tasks.create", () => {
    input.current?.focus();
  });

  // A task that left the list (done, handed over) closes its detail.
  const loaded = mine.isSuccess && away.isSuccess;
  useEffect(() => {
    if (loaded && taskId !== undefined && open === undefined) {
      void navigate({ to: `/w/${workspaceId}/tasks` });
    }
  }, [loaded, taskId, open, navigate, workspaceId]);

  const neighbour = (task: TaskView) => {
    const index = order.findIndex((candidate) => candidate.id === task.id);
    setCursor((order[index + 1] ?? order[index - 1])?.id);
  };
  const done = (task: TaskView) => {
    neighbour(task);
    actions.done(task);
  };
  const notNeeded = (task: TaskView) => {
    neighbour(task);
    actions.notNeeded(task);
  };
  // Whether the task is the caller's to change (not one handed to someone else).
  const isMine = (task: TaskView) => me === undefined || task.assignee === me;
  const holdable = (task: TaskView | undefined): task is TaskView =>
    task !== undefined && isMine(task);

  const move = (step: number) => {
    const index = order.findIndex((task) => task.id === selected?.id);
    const next = order[Math.min(order.length - 1, Math.max(0, index + step))];
    if (next !== undefined) {
      setCursor(next.id);
      const element: { scrollIntoView?: (options: ScrollIntoViewOptions) => void } | null =
        document.getElementById(rowId(next.id));
      element?.scrollIntoView?.({ block: "nearest" });
    }
  };
  const hasRows = selected !== undefined;
  useShortcut(
    "list.next",
    () => {
      move(1);
    },
    { enabled: hasRows },
  );
  useShortcut(
    "list.previous",
    () => {
      move(-1);
    },
    { enabled: hasRows },
  );
  useShortcut(
    "list.open",
    (event) => {
      // Enter on a control is that control's own.
      const target = event?.target;
      if (
        target instanceof HTMLElement &&
        target !== document.body &&
        target.matches("button, a[href], input, select, textarea")
      ) {
        return;
      }
      if (selected !== undefined) {
        openTask(selected);
      }
    },
    { enabled: hasRows },
  );
  useShortcut(
    "item.done",
    () => {
      if (holdable(subject)) {
        done(subject);
      }
    },
    { enabled: holdable(subject) },
  );
  useShortcut(
    "conversations.resolve_need",
    () => {
      if (holdable(subject)) {
        notNeeded(subject);
      }
    },
    { enabled: holdable(subject) },
  );
  useShortcut(
    "item.snooze",
    () => {
      if (holdable(subject)) {
        if (taskId !== subject.id) {
          openTask(subject);
        }
        setOfferLater(true);
      }
    },
    { enabled: holdable(subject) },
  );
  useShortcut("overlay.close", closeDetail, { enabled: taskId !== undefined });

  const dueChip = (task: TaskView): TaskRowChip | undefined => {
    const due = dueText(task, t, i18n.language, clock);
    return due === undefined
      ? undefined
      : { text: due.text, tone: due.late ? "danger" : "neutral" };
  };
  const planChip = (task: TaskView): TaskRowChip | undefined => {
    const plan = planText(task, t, i18n.language, clock);
    return plan === undefined ? undefined : { text: plan };
  };
  const row = (task: TaskView) => {
    const source = sourceOf(task);
    const chips = [
      dueChip(task),
      planChip(task),
      isMine(task) ? undefined : { text: t("row.with", { name: nameOf(task.assignee) }) },
    ].filter((chip): chip is TaskRowChip => chip !== undefined);
    return (
      <TaskRow
        key={task.id}
        id={rowId(task.id)}
        title={task.title}
        kind={t(task.verb === "pay" ? "add.pay" : "add.do")}
        meta={source === undefined ? t("row.own") : t("row.fromEmail", { name: source.name })}
        chips={chips}
        selected={task.id === selected?.id}
        current={task.id === taskId}
        checkLabel={t(task.verb === "pay" ? "row.markPaid" : "row.markDone", { title: task.title })}
        {...(isMine(task)
          ? {
              onCheck: () => {
                done(task);
              },
            }
          : {})}
        {...(me !== undefined && task.handed_by === me && !isMine(task)
          ? {
              action: (
                <Button
                  variant="link"
                  onClick={() => {
                    actions.takeBack(task, me);
                  }}
                >
                  {t("detail.takeBack")}
                </Button>
              ),
            }
          : {})}
        onOpen={() => {
          openTask(task);
        }}
      />
    );
  };

  const loading = mine.isPending || away.isPending;
  const count = own.length;
  const nothing = !loading && !mine.isError && all.length === 0;

  return (
    <>
      <ListPane label={t("list.label")} narrow={taskId === undefined ? "shown" : "hidden"}>
        <PaneHeader>
          <Heading level={1} size="pane">
            {t("title")}
          </Heading>
          <Spacer />
          <Text tone="tertiary" size="sm">
            {t("subtitle", { open: count, week: dueThisWeek(own, clock) })}
          </Text>
        </PaneHeader>
        <QuickAdd inputRef={input} now={now} onAdd={(body) => actions.add(body)} />
        {loading ? <SkeletonList rows={4} /> : null}
        {mine.isError ? (
          <EmptyState
            title={t("error.title")}
            visor="thinking"
            action={
              <Button
                onClick={() => {
                  void mine.refetch();
                }}
              >
                {t("error.retry")}
              </Button>
            }
          >
            {t("error.body")}
          </EmptyState>
        ) : null}
        {nothing ? (
          <EmptyState
            title={t("empty.title")}
            visor="happy"
            visorSize="lg"
            action={
              <Button
                variant="primary"
                icon="plus"
                shortcut="tasks.create"
                onClick={() => {
                  input.current?.focus();
                }}
              >
                {t("empty.add")}
              </Button>
            }
          >
            {t("empty.body")}
          </EmptyState>
        ) : null}
        <div className="flex flex-col">
          {GROUPS.map((group) => {
            const tasks = groups.get(group) ?? [];
            if (tasks.length === 0) {
              return null;
            }
            return (
              <QueueGroup
                key={group}
                signal={SIGNAL[group]}
                name={t(`groups.${group}`)}
                count={tasks.length}
                action={
                  group === "later" ? (
                    <Button
                      variant="ghost"
                      icon={laterOpen ? "chevron-up" : "chevron-down"}
                      aria-expanded={laterOpen}
                      onClick={() => {
                        setLaterOpen((shown) => !shown);
                      }}
                    >
                      {t(laterOpen ? "list.hideLater" : "list.showLater")}
                    </Button>
                  ) : undefined
                }
              >
                {group === "later" && !laterOpen ? null : tasks.map(row)}
              </QueueGroup>
            );
          })}
        </div>
      </ListPane>
      <ReaderPane label={t("detail.label")} narrow={taskId === undefined ? "hidden" : "shown"}>
        {open === undefined ? (
          <div className="flex flex-col items-center gap-2 p-6">
            <EmptyState title={t("detail.empty")} visor="idle" visorSize="lg">
              {t("detail.emptyBody")}
            </EmptyState>
          </div>
        ) : (
          <TaskDetail
            // Each task opens with its own fields.
            key={open.id}
            workspaceId={workspaceId}
            task={open}
            me={me}
            source={sourceOf(open)}
            actions={actions}
            laterOpen={offerLater}
            onLaterOpen={setOfferLater}
            onBack={closeDetail}
            now={now}
          />
        )}
      </ReaderPane>
    </>
  );
}

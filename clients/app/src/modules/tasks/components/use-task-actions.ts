import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";

import { problemCode } from "~/lib/api";
import { useUndoToast } from "~/lib/ui";

import {
  editOf,
  patchTask,
  useRefreshTasks,
  useTaskCalls,
  type NewTask,
  type PlanChange,
  type TaskView,
} from "../api/queries";
import { isLater, whenLabel } from "../model";

/** `t` for keys only known at runtime. */
type Translate = (key: string, options?: Record<string, unknown>) => string;

/** One change to a task: what it shows at once, what it sends, how it is taken back. */
interface Change {
  /** What the toast says: "Done · Send the signed lease". */
  label: string;
  /** The task as the change leaves it, shown before the server answers. */
  optimistic: (task: TaskView) => TaskView;
  forward: (seen: TaskView) => Promise<TaskView>;
  backward: (seen: TaskView) => Promise<TaskView>;
}

/** The call that puts `task`'s plan back as it is. */
export function planOf(task: TaskView, now: Date): PlanChange {
  if (task.plan === "date" && task.plan_on !== null && task.plan_on !== undefined) {
    return { plan: "date", plan_on: task.plan_on };
  }
  if (isLater(task, now) && task.later_until !== null && task.later_until !== undefined) {
    return { plan: "later", later_until: task.later_until };
  }
  return { plan: "none" };
}

/** A plan change as the task shows it before the server answers. */
const planned = (change: PlanChange) => (task: TaskView) =>
  ({
    ...task,
    plan: change.plan,
    plan_on: change.plan === "date" ? change.plan_on : null,
    later_until: change.plan === "later" ? change.later_until : null,
  }) satisfies TaskView;

export interface TaskActionOptions {
  /** After every call settles (the page refetches). */
  onChanged?: (() => void) | undefined;
  /** Pins the clock (stories and tests). */
  now?: Date | undefined;
  /** A change was undone (Home shows the item again at once). */
  onUndo?: ((task: TaskView) => void) | undefined;
  /** An undone change was redone. */
  onRedo?: ((task: TaskView) => void) | undefined;
  /** The server refused or failed: the task is as it was. */
  onFailed?: ((task: TaskView) => void) | undefined;
}

/**
 * A member's changes to their tasks: Done (Paid), Later, a day, a due date, a
 * new title, a hand-off, each at once, each with an Undo, each rolled back
 * with a plain sentence when the server refuses.
 */
export function useTaskActions(
  workspaceId: string,
  { onChanged, now, onUndo, onRedo, onFailed }: TaskActionOptions = {},
) {
  const { t: typed, i18n } = useTranslation("tasks");
  const t = typed as unknown as Translate;
  const undo = useUndoToast();
  const queryClient = useQueryClient();
  const calls = useTaskCalls(workspaceId);
  const refresh = useRefreshTasks(workspaceId);
  const clock = () => now ?? new Date();

  const act = (task: TaskView, change: Change) => {
    let latest = task;
    let queue: Promise<unknown> = Promise.resolve();
    // Calls on one task go one after the other, each with the revision the
    // one before answered.
    const send = (call: (seen: TaskView) => Promise<TaskView>) => {
      const result = queue
        .then(() => call(latest))
        .then((view) => {
          latest = view;
          return view;
        });
      queue = result.catch(() => undefined);
      return result;
    };
    const show = (next: (current: TaskView) => TaskView) => {
      patchTask(queryClient, workspaceId, task.id, next);
    };
    const settled = () => {
      refresh();
      onChanged?.();
    };
    const failed = (error: unknown, retry: () => void, entry?: number) => {
      if (entry !== undefined) {
        undo.retract(entry);
      }
      // As it was: the server's view, fetched again.
      onFailed?.(task);
      settled();
      if (problemCode(error) === "task_changed") {
        undo.fail({ label: t("toast.changed", { title: task.title }) });
      } else {
        undo.fail({ label: t("toast.failed", { title: task.title }), retry });
      }
    };
    const run = (call: (seen: TaskView) => Promise<TaskView>, entry?: number) => {
      void send(call).then(settled, (error: unknown) => {
        failed(
          error,
          () => {
            onRedo?.(task);
            show(change.optimistic);
            run(call);
          },
          entry,
        );
      });
    };
    show(change.optimistic);
    const entry = undo.push({
      label: change.label,
      undo: () => {
        onUndo?.(task);
        show(() => task);
        run(change.backward);
      },
      redo: () => {
        onRedo?.(task);
        show(change.optimistic);
        run(change.forward);
      },
    });
    run(change.forward, entry);
  };

  const dayText = (day: string) => whenLabel(`${day}T12:00:00`, i18n.language);

  return {
    /** Done, or Paid for a bill. */
    done: (task: TaskView) => {
      act(task, {
        label: t(task.verb === "pay" ? "toast.paid" : "toast.done", { title: task.title }),
        optimistic: (current) => ({ ...current, state: "done" }),
        forward: calls.done,
        backward: calls.reopen,
      });
    },
    notNeeded: (task: TaskView) => {
      act(task, {
        label: t("toast.notNeeded", { title: task.title }),
        optimistic: (current) => ({ ...current, state: "not_needed" }),
        forward: calls.notNeeded,
        backward: calls.reopen,
      });
    },
    /** Put it off until `until`: it leaves the lists until then. */
    later: (task: TaskView, until: Date) => {
      const change: PlanChange = { plan: "later", later_until: until.toISOString() };
      act(task, {
        label: t("toast.later", { title: task.title, when: whenLabel(until, i18n.language) }),
        optimistic: planned(change),
        forward: (seen) => calls.plan(seen, change),
        backward: (seen) => calls.plan(seen, planOf(task, clock())),
      });
    },
    /** Plan a day (`undefined` clears the plan). */
    planDay: (task: TaskView, day: string | undefined) => {
      const change: PlanChange =
        day === undefined ? { plan: "none" } : { plan: "date", plan_on: day };
      act(task, {
        label:
          day === undefined
            ? t("toast.planCleared", { title: task.title })
            : t("toast.planned", { title: task.title, date: dayText(day) }),
        optimistic: planned(change),
        forward: (seen) => calls.plan(seen, change),
        backward: (seen) => calls.plan(seen, planOf(task, clock())),
      });
    },
    /** Change the due day (`null` clears it). */
    setDue: (task: TaskView, day: string | null) => {
      const before = editOf(task);
      act(task, {
        label: t("toast.dueChanged", { title: task.title }),
        optimistic: (current) => ({ ...current, due_on: day, due_at: null }),
        forward: (seen) => calls.update(seen, { ...editOf(seen), due_on: day, due_at: null }),
        backward: (seen) =>
          calls.update(seen, { ...editOf(seen), due_on: before.due_on, due_at: before.due_at }),
      });
    },
    rename: (task: TaskView, title: string) => {
      const before = editOf(task);
      act(task, {
        label: t("toast.renamed", { title }),
        optimistic: (current) => ({ ...current, title }),
        forward: (seen) => calls.update(seen, { ...editOf(seen), title }),
        backward: (seen) => calls.update(seen, { ...editOf(seen), title: before.title }),
      });
    },
    handOff: (task: TaskView, member: { id: string; name: string }) => {
      act(task, {
        label: t("toast.handedOff", { title: task.title, name: member.name }),
        optimistic: (current) => ({ ...current, assignee: member.id, handed_by: current.assignee }),
        forward: (seen) => calls.handOff(seen, member.id),
        backward: calls.takeBack,
      });
    },
    takeBack: (task: TaskView, giver: string) => {
      act(task, {
        label: t("toast.takenBack", { title: task.title }),
        optimistic: (current) => ({ ...current, assignee: giver, handed_by: null }),
        forward: calls.takeBack,
        backward: (seen) => calls.handOff(seen, task.assignee),
      });
    },
    /** Add a task; Undo marks it not needed. */
    add: async (body: NewTask, label?: string): Promise<TaskView | undefined> => {
      const create = async (): Promise<TaskView | undefined> => {
        try {
          const made = await calls.create(body);
          refresh();
          onChanged?.();
          let latest = made;
          undo.push({
            label: label ?? t("add.added"),
            undo: () => {
              void calls
                .notNeeded(latest)
                .then((view) => {
                  latest = view;
                })
                .finally(refresh);
            },
            redo: () => {
              void calls
                .reopen(latest)
                .then((view) => {
                  latest = view;
                })
                .finally(refresh);
            },
          });
          return made;
        } catch {
          undo.fail({
            label: t("add.failed", { title: body.title }),
            retry: () => {
              void create();
            },
          });
          return undefined;
        }
      };
      return create();
    },
  };
}

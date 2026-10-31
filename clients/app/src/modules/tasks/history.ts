import type { TaskHistoryEntry, TaskView } from "./api/queries";
import { heldLabel } from "./hold";
import { clockTime, dayLabel, shortDay, whenLabel } from "./model";
import type { TaskTranslate } from "./task-text";

/** Who is who in a task's history: the caller, the team, where Scout's email came from. */
export interface HistoryContext {
  t: TaskTranslate;
  locale: string;
  /** The caller. */
  me: string | undefined;
  /** A member's name, when the member list has them. */
  memberName: (id: string) => string | undefined;
  /** Who the task's email is from, for "Scout made it from Marta's email". */
  sourceName: string | undefined;
}

/** A member's name, or "A teammate" when the list does not have them. */
function nameOf(context: HistoryContext, id: string | null | undefined): string {
  const name = id === null || id === undefined ? undefined : context.memberName(id);
  return name ?? context.t("history.teammate");
}

/** The sentence for one entry of the history, with who did it first. */
export function historyLine(entry: TaskHistoryEntry, verb: TaskView["verb"], c: HistoryContext) {
  const { t, locale } = c;
  const detail = entry.detail;
  const mine = entry.actor === "member" && entry.actor_member === c.me && c.me !== undefined;
  const who =
    entry.actor === "scout"
      ? t("history.scout")
      : entry.actor === "calendar"
        ? t("history.calendar")
        : mine
          ? ""
          : nameOf(c, entry.actor_member);
  const say = (key: string, options: Record<string, unknown> = {}) =>
    t(`history.${key}.${mine ? "you" : "other"}`, { who, ...options });
  const changed = () => say("changed");
  const due =
    detail.due_on === null || detail.due_on === undefined
      ? ""
      : t("history.dueSuffix", { date: dayLabel(detail.due_on, locale) });

  switch (entry.what) {
    case "created":
      return entry.actor === "scout" && c.sourceName !== undefined
        ? t("history.created.scoutFrom", { name: c.sourceName }) + due
        : say("created") + due;
    case "edited": {
      const fields = (detail.fields ?? []).map((field) => t(`history.field.${field}`));
      return fields.length === 0
        ? changed()
        : say("edited", {
            fields: new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(
              fields,
            ),
          });
    }
    case "due_changed":
      return detail.due_on === null || detail.due_on === undefined
        ? say("dueRemoved")
        : say("dueSet", { date: dayLabel(detail.due_on, locale) });
    case "planned":
      if (detail.plan === "date" && detail.plan_on !== null && detail.plan_on !== undefined) {
        return say("planned", { day: dayLabel(detail.plan_on, locale) });
      }
      if (
        detail.plan === "later" &&
        detail.later_until !== null &&
        detail.later_until !== undefined
      ) {
        return say("later", { date: whenLabel(detail.later_until, locale) });
      }
      return detail.plan === "none" ? say("unplanned") : changed();
    case "done":
      return say(verb === "pay" ? "paid" : "done");
    case "not_needed":
      return say("notNeeded");
    case "reopened":
      return say("reopened");
    case "handed_off":
      return detail.member === c.me && c.me !== undefined
        ? t("history.handedOff.toYou", { who })
        : say("handedOff", { member: nameOf(c, detail.member) });
    case "taken_back":
      return detail.member === c.me && c.me !== undefined
        ? t("history.takenBack.fromYou", { who })
        : say("takenBack", { member: nameOf(c, detail.member) });
    case "held": {
      if (detail.held_from === null || detail.held_from === undefined) {
        return changed();
      }
      const when = heldLabel(
        { starts: detail.held_from, ends: detail.held_to ?? detail.held_from },
        locale,
      );
      return entry.actor === "calendar" ? t("history.held.moved", { when }) : say("held", { when });
    }
    case "released":
      return say("released");
    case "replied":
      return t("history.replied");
    case "reply_answered":
      return say("replyAnswered");
    case "reply_unanswered":
      return say("replyUnanswered");
    default:
      return changed();
  }
}

/** When an entry happened, for the left of its line: "27 Oct, 14:20". */
export function historyStamp(at: string, locale: string): string {
  return `${shortDay(at, locale)}, ${clockTime(at, locale)}`;
}

/** The entries shown when more than `FOLD` exist: the first and the latest ones. */
export const FOLD = 8;
const LATEST = 6;

/** Which entries to show: all, or the first and the latest `LATEST` while folded. */
export function visibleHistory<T>(entries: readonly T[], expanded: boolean): T[] {
  return expanded || entries.length <= FOLD
    ? [...entries]
    : [...entries.slice(0, 1), ...entries.slice(-LATEST)];
}

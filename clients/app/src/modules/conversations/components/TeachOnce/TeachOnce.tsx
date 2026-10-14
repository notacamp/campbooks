import {
  createContext,
  use,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";

import { isTypingTarget, useShortcut } from "~/lib/shortcuts";
import { Button, ChoiceCards, Line, Offer, Progress, useUndoToast, VisuallyHidden } from "~/lib/ui";

import { useInboxes, type ConversationSummary } from "../../api/queries";
import {
  dismissSuggestions,
  refreshConversations,
  suggestRules,
  useRule,
  type RuleException,
  type RuleSuggestions,
  type TaughtAction,
} from "../../api/rules";
import { listTime, streamName, type Translate } from "../../model";
import {
  likenessTitle,
  ruleSentence,
  ruleStream,
  taughtKind,
  taughtStream,
} from "../../rules-text";

import { useRuleActions } from "./use-rule-actions";

/** Sample subjects an offer shows. */
const SAMPLES = 3;

/** What the offer said it would change: the run's title says the same number. */
interface Expected {
  count: number;
  complete: boolean;
}

type Step =
  | { step: "idle" }
  | {
      step: "asking";
      conversation: ConversationSummary;
      action: TaughtAction;
      offer: RuleSuggestions;
      chosen: number | null;
    }
  | {
      step: "running";
      ruleId: string;
      action: TaughtAction;
      expected: Expected;
      reverting: boolean;
    }
  | { step: "question"; ruleId: string; recent: number };

export interface TeachOnceApi {
  /** After a member acted on one conversation: offer to do the same to its lookalikes. */
  after: (conversation: ConversationSummary, action: TaughtAction) => void;
  /** A member changed back what a rule did: ask about the rule at the second time. */
  exception: (exception: RuleException | null | undefined) => void;
  /**
   * An offer or a question is up and holds the choice keys (1): screens
   * behind it leave theirs unbound and unhinted meanwhile.
   */
  offering: boolean;
}

const TeachOnceContext = createContext<TeachOnceApi | null>(null);

/** The teach-once API inside the inbox; `null` elsewhere (Home, search results). */
export function useTeachOnce(): TeachOnceApi | null {
  return use(TeachOnceContext);
}

/**
 * Teach once (campbooks-cloud#125): after a member moves, archives, marks read
 * or clears one conversation, offer to do the same to every one like it, now
 * and as new ones arrive; then show the rule's run with Undo all, and ask
 * about a rule the member keeps changing back.
 */
export function TeachOnceProvider({
  workspaceId,
  children,
}: {
  workspaceId: string;
  children: ReactNode;
}) {
  const [state, setState] = useState<Step>({ step: "idle" });
  const [announcement, setAnnouncement] = useState("");
  const asked = useRef(0);

  const after = useCallback(
    (conversation: ConversationSummary, action: TaughtAction) => {
      const ask = asked.current + 1;
      asked.current = ask;
      void suggestRules(workspaceId, conversation.id, action)
        .then((offer) => {
          // A later action asked again meanwhile: that one wins.
          if (asked.current !== ask || offer.suggestions.length === 0) {
            return;
          }
          setState({
            step: "asking",
            conversation,
            action,
            offer,
            chosen: offer.default ?? null,
          });
        })
        // An offer is a nicety: if asking fails, there is simply none.
        .catch(() => null);
    },
    [workspaceId],
  );

  const exception = useCallback((found: RuleException | null | undefined) => {
    if (found !== null && found !== undefined && found.recent_exceptions >= 2) {
      setState({ step: "question", ruleId: found.rule_id, recent: found.recent_exceptions });
    }
  }, []);

  const offering = state.step === "asking" || state.step === "question";
  const api = useMemo<TeachOnceApi>(
    () => ({ after, exception, offering }),
    [after, exception, offering],
  );

  return (
    <TeachOnceContext value={api}>
      {children}
      {state.step === "idle" ? null : (
        <TeachOffer
          workspaceId={workspaceId}
          state={state}
          setState={setState}
          announce={setAnnouncement}
        />
      )}
      {/* Mounted for good, so what the offer says is read out as it changes. */}
      <VisuallyHidden>
        <span role="status" aria-live="polite">
          {announcement}
        </span>
      </VisuallyHidden>
    </TeachOnceContext>
  );
}

interface StepProps<S extends Step["step"]> {
  workspaceId: string;
  state: Extract<Step, { step: S }>;
  setState: (next: Step) => void;
  announce: (text: string) => void;
  /** Whether focus is inside the offer: Esc is the offer's only then. */
  focused: boolean;
}

function TeachOffer({
  workspaceId,
  state,
  setState,
  announce,
}: {
  workspaceId: string;
  state: Exclude<Step, { step: "idle" }>;
  setState: (next: Step) => void;
  announce: (text: string) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [focused, setFocused] = useState(false);
  // When the offer appears or changes step, bring focus to its first action,
  // unless the member is typing somewhere (never take focus from a composer).
  useEffect(() => {
    const active = document.activeElement;
    const within = ref.current?.contains(active) === true;
    if (isTypingTarget(active) && !within) {
      return;
    }
    ref.current?.querySelector<HTMLButtonElement>("button:not([disabled])")?.focus();
  }, [state.step]);

  // Whether focus is inside the offer, followed on the document.
  useEffect(() => {
    const follow = (event: FocusEvent) => {
      const now = event.type === "focusin" ? event.target : event.relatedTarget;
      setFocused(now instanceof Node && ref.current?.contains(now) === true);
    };
    document.addEventListener("focusin", follow);
    document.addEventListener("focusout", follow);
    return () => {
      document.removeEventListener("focusin", follow);
      document.removeEventListener("focusout", follow);
    };
  }, []);

  const common = { workspaceId, setState, announce, focused };
  return (
    <div ref={ref} className="contents">
      {state.step === "asking" ? <Asking {...common} state={state} /> : null}
      {state.step === "running" ? <Running {...common} state={state} /> : null}
      {state.step === "question" ? <Question {...common} state={state} /> : null}
    </div>
  );
}

function useT(): { t: Translate; locale: string; number: (n: number) => string } {
  const { t, i18n } = useTranslation("conversations");
  const locale = i18n.language;
  return {
    t: t as unknown as Translate,
    locale,
    number: (n) => new Intl.NumberFormat(locale).format(n),
  };
}

/** Say `text` through the polite live region whenever it changes. */
function useAnnounce(announce: (text: string) => void, text: string) {
  useEffect(() => {
    announce(text);
  }, [announce, text]);
}

/** "Move all 1,000 like this to Updates, and new ones too?" */
function Asking({ workspaceId, state, setState, announce, focused }: StepProps<"asking">) {
  const { t, locale, number } = useT();
  const undo = useUndoToast();
  const inboxes = useInboxes(workspaceId);
  const { create, removeOnce } = useRuleActions(workspaceId);
  const { offer, conversation, action } = state;
  const chosen = state.chosen === null ? undefined : offer.suggestions[state.chosen];
  const kind = taughtKind(action);
  const stream = taughtStream(t, action);
  const inbox = inboxes.data?.find((item) => item.id === conversation.inbox_id);
  const count = chosen?.preview.would_change ?? 0;
  const complete = chosen?.preview.complete ?? true;
  const counts = { count, n: number(count), stream };
  const skipped = chosen?.preview.skipped_suspicious ?? 0;

  let title: string;
  let accept: string;
  if (chosen === undefined) {
    title = t(`rules.offer.choose.${kind}`, { stream });
    accept = t(`rules.offer.acceptAtLeast.${kind}`);
  } else if (complete) {
    title = t(`rules.offer.question.${kind}`, counts);
    accept = t(`rules.offer.accept.${kind}`, counts);
  } else {
    title = t(`rules.offer.questionAtLeast.${kind}`, counts);
    accept = t(`rules.offer.acceptAtLeast.${kind}`);
  }
  useAnnounce(announce, title);

  const dismiss = () => {
    // "Just this one" turns down every likeness shown, so none comes back.
    const keys = offer.suggestions.map((suggestion) => suggestion.key);
    dismissSuggestions(workspaceId, conversation.id, keys).catch(() => {
      undo.notify(t("rules.offer.dismissFailed"));
    });
    setState({ step: "idle" });
  };
  const accept_ = () => {
    if (chosen === undefined || create.isPending) {
      return;
    }
    create.mutate(
      {
        inboxId: conversation.inbox_id,
        conditions: chosen.conditions,
        actions: chosen.actions,
        fromConversation: conversation.id,
      },
      {
        onSuccess: ({ rule }) => {
          setState({
            step: "running",
            ruleId: rule.id,
            action,
            expected: { count, complete },
            reverting: false,
          });
          undo.push({
            label: t("rules.toast.made", { rule: ruleSentence(t, locale, rule) }),
            undo: () => {
              removeOnce(rule.id, true);
            },
          });
        },
        onError: () => {
          undo.notify(t("rules.run.failed"));
          setState({ step: "idle" });
        },
      },
    );
  };
  useShortcut("item.choice1", accept_, { enabled: chosen !== undefined });
  useShortcut("overlay.close", dismiss, { enabled: focused });

  const samples = chosen?.preview.samples.slice(0, SAMPLES) ?? [];
  const more = count - samples.length;
  return (
    <Offer
      title={title}
      detail={[
        skipped > 0
          ? t("rules.offer.suspicious", { count: skipped, n: number(skipped) })
          : t("rules.offer.safe"),
        complete ? "" : t("rules.offer.partial"),
        offer.shared && inbox !== undefined
          ? t("rules.offer.shared", { inbox: inbox.address })
          : "",
      ]
        .filter((line) => line !== "")
        .join(" ")}
      actions={
        <>
          <Button
            variant="primary"
            shortcut="item.choice1"
            disabled={chosen === undefined || create.isPending}
            onClick={accept_}
          >
            {accept}
          </Button>
          <Button variant="ghost" shortcut="overlay.close" onClick={dismiss}>
            {t("rules.offer.justThis")}
          </Button>
        </>
      }
      foot={t("rules.offer.foot")}
    >
      {offer.suggestions.length > 1 ? (
        <ChoiceCards
          label={t("rules.offer.which")}
          hideLabel
          value={state.chosen === null ? null : String(state.chosen)}
          onChange={(value) => {
            setState({ ...state, chosen: Number(value) });
          }}
          options={offer.suggestions.map((suggestion, index) => ({
            value: String(index),
            title: likenessTitle(t, suggestion),
            description: number(suggestion.preview.would_change),
          }))}
        />
      ) : null}
      {samples.length === 0 ? null : (
        <div className="flex flex-col">
          {samples.map((sample) => (
            <Line key={sample.conversation_id} trailing={listTime(sample.sent_at, locale)}>
              {sample.subject}
            </Line>
          ))}
          {more > 0 && complete ? <Line>{t("rules.offer.more", { n: number(more) })}</Line> : null}
        </div>
      )}
    </Offer>
  );
}

/** The rule's run: progress, then what it did, with Undo all. */
function Running({ workspaceId, state, setState, announce, focused }: StepProps<"running">) {
  const { t, number } = useT();
  const queryClient = useQueryClient();
  const rule = useRule(workspaceId, state.ruleId);
  const { removeRule, removing } = useRuleActions(workspaceId);
  const data = rule.data;
  const run = data?.run ?? undefined;
  const stream = streamName(t, ruleStream(data?.actions ?? []) ?? "primary");
  // Our own Undo all removes the rule too; only someone else's removal is news.
  const removedElsewhere = data?.state === "removed" && !state.reverting;
  const reverting = state.reverting;
  const revertRun = reverting && run?.kind === "revert" ? run : undefined;
  const applyRun = !reverting && run?.kind === "apply" ? run : undefined;
  const shown = reverting ? revertRun : applyRun;
  const kind = reverting ? "revert" : taughtKind(state.action);
  const ended = shown !== undefined && shown.state !== "running";

  // Rows say what the rule did to them: refresh them once a run ends.
  useEffect(() => {
    if (ended) {
      void refreshConversations(queryClient, workspaceId);
    }
  }, [ended, queryClient, workspaceId]);

  const close = () => {
    setState({ step: "idle" });
  };
  useShortcut("overlay.close", close, { enabled: focused });

  let title: string;
  let detail: string | undefined;
  let progress: { value: number; max: number; line: string } | undefined;
  if (rule.isError) {
    title = t("rules.run.loadFailed");
  } else if (removedElsewhere || shown?.state === "cancelled") {
    const changed = applyRun?.changed ?? 0;
    title =
      changed > 0
        ? t("rules.run.stopped", { count: changed, n: number(changed) })
        : removedElsewhere
          ? t("rules.run.removed")
          : t("rules.run.stoppedNone");
  } else if (reverting && revertRun === undefined) {
    // Removed; the run putting mail back has not shown up yet.
    title = t("rules.run.undoing");
    progress = { value: 0, max: 0, line: t("rules.run.counting") };
  } else if (shown?.counted !== true) {
    title = reverting ? t("rules.run.undoing") : t("rules.run.finding");
    progress = { value: 0, max: 0, line: t("rules.run.counting") };
  } else if (ended) {
    title = t(`rules.run.done.${kind}`, {
      count: shown.changed,
      n: number(shown.changed),
      stream,
    });
    detail =
      shown.skipped_suspicious > 0
        ? t("rules.run.leftAlone", {
            count: shown.skipped_suspicious,
            n: number(shown.skipped_suspicious),
          })
        : undefined;
  } else {
    // The same number the offer gave, when it had them all.
    const count =
      !reverting && state.expected.complete && state.expected.count > 0
        ? state.expected.count
        : shown.total;
    title = t(`rules.run.working.${kind}`, { count, n: number(count), stream });
    detail = t("rules.run.keepWorking");
    progress = {
      value: shown.done,
      max: shown.total,
      line: t("rules.run.progress", { done: number(shown.done), total: number(shown.total) }),
    };
  }
  const settled = rule.isError || removedElsewhere || ended || shown?.state === "cancelled";
  useAnnounce(announce, settled ? title : "");

  const canUndo =
    !reverting &&
    !removedElsewhere &&
    !rule.isError &&
    data?.may_change === true &&
    applyRun !== undefined &&
    applyRun.state !== "cancelled";
  const undoAll = () => {
    if (data === undefined || !canUndo || removing) {
      return;
    }
    removeRule(data, {
      restore: true,
      failure: t("rules.run.undoFailed"),
      onDone: () => {
        setState({ ...state, reverting: true });
      },
    });
  };

  return (
    <Offer
      visor={settled ? (rule.isError ? "unsure" : "happy") : "thinking"}
      title={title}
      detail={detail}
      actions={
        <>
          {canUndo ? (
            <Button variant={ended ? "primary" : "secondary"} disabled={removing} onClick={undoAll}>
              {t("rules.run.undoAll")}
            </Button>
          ) : null}
          <Button variant="ghost" shortcut="overlay.close" onClick={close}>
            {t("rules.run.close")}
          </Button>
        </>
      }
    >
      {progress === undefined ? null : (
        <Progress label={title} value={progress.value} max={progress.max}>
          {progress.line}
        </Progress>
      )}
    </Offer>
  );
}

/** "Changed back 2 times in the last 30 days. Keep the rule …?" */
function Question({ workspaceId, state, setState, announce, focused }: StepProps<"question">) {
  const { t, locale, number } = useT();
  const undo = useUndoToast();
  const rule = useRule(workspaceId, state.ruleId);
  const { removeRule, removing } = useRuleActions(workspaceId);
  const data = rule.data;
  const keep = () => {
    setState({ step: "idle" });
  };
  // A question about a rule that cannot be read is no question: say so.
  useEffect(() => {
    if (rule.isError) {
      undo.notify(t("rules.run.loadFailed"));
      setState({ step: "idle" });
    }
  }, [rule.isError, setState, t, undo]);
  const canDrop = data?.may_change === true && data.state !== "removed";
  const drop = () => {
    if (data === undefined || !canDrop || removing) {
      return;
    }
    removeRule(data, {
      restore: false,
      onDone: () => {
        setState({ step: "idle" });
      },
    });
  };
  useShortcut("item.choice1", drop, { enabled: canDrop });
  useShortcut("overlay.close", keep, { enabled: focused });
  const title =
    data === undefined
      ? ""
      : t("rules.exception.question", {
          count: state.recent,
          n: number(state.recent),
          rule: ruleSentence(t, locale, data),
        });
  useAnnounce(announce, title);
  if (data === undefined) {
    return null;
  }
  return (
    <Offer
      visor="unsure"
      title={title}
      actions={
        <>
          {canDrop ? (
            <Button variant="primary" shortcut="item.choice1" disabled={removing} onClick={drop}>
              {t("rules.exception.remove")}
            </Button>
          ) : null}
          <Button variant="ghost" shortcut="overlay.close" onClick={keep}>
            {t("rules.exception.keep")}
          </Button>
        </>
      }
    />
  );
}

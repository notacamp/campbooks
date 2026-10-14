import { createContext, use, useCallback, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import { Button, ChoiceCards, Line, Offer, Progress, useUndoToast } from "~/lib/ui";

import { useInboxes, type ConversationSummary } from "../../api/queries";
import {
  dismissSuggestion,
  suggestRules,
  useRule,
  useRuleMutations,
  type RuleException,
  type RuleSuggestions,
  type RuleView,
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

/** Sample subjects an offer shows. */
const SAMPLES = 3;

type Step =
  | { step: "idle" }
  | {
      step: "asking";
      conversation: ConversationSummary;
      action: TaughtAction;
      offer: RuleSuggestions;
      chosen: number | null;
    }
  | { step: "running"; ruleId: string; action: TaughtAction; reverting: boolean }
  | { step: "question"; ruleId: string; recent: number };

export interface TeachOnceApi {
  /** After a member acted on one conversation: offer to do the same to its lookalikes. */
  after: (conversation: ConversationSummary, action: TaughtAction) => void;
  /** A member changed back what a rule did: ask about the rule at the second time. */
  exception: (exception: RuleException | null | undefined) => void;
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

  const api = useMemo<TeachOnceApi>(() => ({ after, exception }), [after, exception]);

  return (
    <TeachOnceContext value={api}>
      {children}
      {state.step === "idle" ? null : (
        <TeachOffer workspaceId={workspaceId} state={state} setState={setState} />
      )}
    </TeachOnceContext>
  );
}

function TeachOffer({
  workspaceId,
  state,
  setState,
}: {
  workspaceId: string;
  state: Exclude<Step, { step: "idle" }>;
  setState: (next: Step) => void;
}) {
  switch (state.step) {
    case "asking": {
      return <Asking workspaceId={workspaceId} state={state} setState={setState} />;
    }
    case "running": {
      return <Running workspaceId={workspaceId} state={state} setState={setState} />;
    }
    case "question": {
      return <Question workspaceId={workspaceId} state={state} setState={setState} />;
    }
  }
}

function useT(): { t: Translate; locale: string } {
  const { t, i18n } = useTranslation("conversations");
  return { t: t as unknown as Translate, locale: i18n.language };
}

/** "Move all 1,000 like this to Updates, and new ones too?" */
function Asking({
  workspaceId,
  state,
  setState,
}: {
  workspaceId: string;
  state: Extract<Step, { step: "asking" }>;
  setState: (next: Step) => void;
}) {
  const { t, locale } = useT();
  const undo = useUndoToast();
  const inboxes = useInboxes(workspaceId);
  const { create, remove } = useRuleMutations(workspaceId);
  const { offer, conversation, action } = state;
  const chosen = state.chosen === null ? undefined : offer.suggestions[state.chosen];
  const kind = taughtKind(action);
  const count = new Intl.NumberFormat(locale).format(chosen?.preview.would_change ?? 0);
  const stream = taughtStream(t, action);
  const inbox = inboxes.data?.find((item) => item.id === conversation.inbox_id);
  const skipped = chosen?.preview.skipped_suspicious ?? 0;

  const dismiss = () => {
    const key = chosen?.key ?? offer.suggestions[0]?.key;
    if (key !== undefined) {
      void dismissSuggestion(workspaceId, conversation.id, key).catch(() => null);
    }
    setState({ step: "idle" });
  };
  const accept = () => {
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
          setState({ step: "running", ruleId: rule.id, action, reverting: false });
          undo.push({
            label: t("rules.toast.made", { rule: ruleSentence(t, locale, rule) }),
            undo: () => {
              remove.mutate({ ruleId: rule.id, restore: true });
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
  useShortcut("item.choice1", accept, { enabled: chosen !== undefined });
  useShortcut("overlay.close", dismiss);

  const samples = chosen?.preview.samples.slice(0, SAMPLES) ?? [];
  const more = (chosen?.preview.would_change ?? 0) - samples.length;
  return (
    <Offer
      title={t(`rules.offer.question.${kind}`, { count, stream })}
      detail={[
        skipped > 0 ? t("rules.offer.suspicious", { count: skipped }) : t("rules.offer.safe"),
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
            onClick={accept}
          >
            {t(`rules.offer.accept.${kind}`, { count, stream })}
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
            description: new Intl.NumberFormat(locale).format(suggestion.preview.would_change),
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
          {more > 0 ? (
            <Line>
              {t("rules.offer.more", { count: new Intl.NumberFormat(locale).format(more) })}
            </Line>
          ) : null}
        </div>
      )}
    </Offer>
  );
}

/** The rule's run: progress, then what it did, with Undo all. */
function Running({
  workspaceId,
  state,
  setState,
}: {
  workspaceId: string;
  state: Extract<Step, { step: "running" }>;
  setState: (next: Step) => void;
}) {
  const { t, locale } = useT();
  const rule = useRule(workspaceId, state.ruleId);
  const { remove } = useRuleMutations(workspaceId);
  const run = rule.data?.run ?? undefined;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const reverting = state.reverting || run?.kind === "revert";
  const kind = reverting ? "revert" : taughtKind(state.action);
  const stream = streamName(t, ruleStream(rule.data?.actions ?? []) ?? "primary");
  const finished = run !== undefined && run.state !== "running";

  const close = () => {
    setState({ step: "idle" });
  };
  const undoAll = () => {
    if (rule.data === undefined || reverting) {
      return;
    }
    remove.mutate({ ruleId: state.ruleId, restore: true });
    setState({ ...state, reverting: true });
  };
  useShortcut("overlay.close", close);

  const total = run?.counted === true ? run.total : undefined;
  let title: string;
  if (finished) {
    title = t(`rules.run.done.${kind}`, { count: number(run.changed), stream });
  } else if (total === undefined) {
    title = t("rules.run.finding");
  } else {
    title = t(`rules.run.working.${kind}`, { count: number(total), stream });
  }
  return (
    <Offer
      visor={finished ? "happy" : "thinking"}
      title={title}
      detail={
        finished
          ? run.skipped_suspicious > 0
            ? t("rules.run.leftAlone", { count: number(run.skipped_suspicious) })
            : undefined
          : t("rules.run.keepWorking")
      }
      actions={
        <>
          {reverting ? null : (
            <Button variant={finished ? "primary" : "secondary"} onClick={undoAll}>
              {t("rules.run.undoAll")}
            </Button>
          )}
          <Button variant="ghost" shortcut="overlay.close" onClick={close}>
            {t("rules.run.close")}
          </Button>
        </>
      }
    >
      {finished || run === undefined ? null : (
        <Progress label={title} value={run.done} max={total ?? 0}>
          {total === undefined
            ? t("rules.run.counting")
            : t("rules.run.progress", { done: number(run.done), total: number(total) })}
        </Progress>
      )}
    </Offer>
  );
}

/** "You changed back 2 this month. Keep the rule …?" */
function Question({
  workspaceId,
  state,
  setState,
}: {
  workspaceId: string;
  state: Extract<Step, { step: "question" }>;
  setState: (next: Step) => void;
}) {
  const { t, locale } = useT();
  const undo = useUndoToast();
  const rule = useRule(workspaceId, state.ruleId);
  const { remove, create } = useRuleMutations(workspaceId);
  const data: RuleView | undefined = rule.data;
  const keep = () => {
    setState({ step: "idle" });
  };
  const drop = () => {
    if (data === undefined) {
      return;
    }
    remove.mutate({ ruleId: data.id, restore: false });
    undo.push({
      label: t("rules.toast.stopped", { rule: ruleSentence(t, locale, data) }),
      undo: () => {
        create.mutate({
          inboxId: data.inbox_id,
          conditions: data.conditions,
          actions: data.actions,
          applyToExisting: false,
        });
      },
    });
    setState({ step: "idle" });
  };
  useShortcut("item.choice1", drop, { enabled: data?.may_change === true });
  useShortcut("overlay.close", keep);
  if (data === undefined) {
    return null;
  }
  return (
    <Offer
      visor="unsure"
      title={t("rules.exception.question", {
        count: state.recent,
        rule: ruleSentence(t, locale, data),
      })}
      actions={
        <>
          {data.may_change ? (
            <Button variant="primary" shortcut="item.choice1" onClick={drop}>
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

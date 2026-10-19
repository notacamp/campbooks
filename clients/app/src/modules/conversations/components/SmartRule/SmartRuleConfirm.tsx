import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Button,
  Checkbox,
  ChoiceCards,
  Dialog,
  RulePreview,
  Text,
  TextField,
  useUndoToast,
  type ChoiceCardOption,
  type RulePreviewSample,
} from "~/lib/ui";

import {
  useCompileGoal,
  useRuleMutations,
  useRulePreview,
  type RuleAction,
  type RuleCondition,
  type RuleNotice,
  type RuleView,
} from "../../api/rules";
import { streamName, type Translate } from "../../model";
import { ruleSentence, ruleStream } from "../../rules-text";

/** Examples the confirmation shows. */
const EXAMPLES = 3;

/** A rule an offer proposes, before the member confirms it. */
export interface SmartRuleDraft {
  /** The inbox it would be saved in. */
  inboxId: string;
  conditions: RuleCondition[];
  actions: RuleAction[];
  /** The dialog's title: "A rule for mail from notifications@github.com". */
  title: string;
  /** Where it came from (an action by default; `suggestion` for Scout noticed). */
  origin?: RuleView["origin"] | undefined;
  /** The conversation it was taught from. */
  fromConversation?: string | undefined;
  /** The "Scout noticed" suggestion it answers. */
  noticeId?: string | undefined;
  /**
   * For a "Scout noticed" draft: the confirmation first asks what should
   * happen to the sender's mail (campbooks-cloud#274). `actions` is Scout's
   * guess; `start` says whether it is chosen from the first (the notice's own
   * button) or nothing is until the member picks ("Something else…").
   */
  goal?:
    | {
        kind: RuleNotice["kind"];
        /** Who the mail is from, as the question says it. */
        sender: string;
        start: "guess" | "none";
      }
    | undefined;
  /** The inbox's address when other members use it: the rule acts for them too. */
  sharedInbox?: string | undefined;
  /**
   * Other senders the member may pick instead (the first is `conditions`):
   * the address, or anyone at its domain.
   */
  choices?:
    | readonly {
        id: string;
        title: string;
        description?: string | undefined;
        /** The dialog's title while it is picked. */
        heading?: string | undefined;
        conditions: RuleCondition[];
      }[]
    | undefined;
}

/** What a member may want done with a sender's mail, instead of Scout's guess. */
type GoalId = "guess" | "highlight" | "newsletters" | "read" | "words";

/** "Highlight them": each new one waits on Home until opened once; nothing is pinned. */
const HIGHLIGHT: RuleAction[] = [{ action: "worth_a_look" }];
/** "File them under Newsletters": off Home, still in the inbox. */
const NEWSLETTERS: RuleAction[] = [
  { action: "move_to_stream", stream: "newsletters" },
  { action: "surface", value: "never" },
];
/** "Mark them read": nothing else changes. */
const MARK_READ: RuleAction[] = [{ action: "mark_read" }];

/** Whether two lists of actions are the same rule. */
function sameActions(a: readonly RuleAction[], b: readonly RuleAction[]): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/**
 * What a goal does: Scout's guess (`guess`), or one of the others; nothing
 * until a goal is chosen.
 */
function goalActions(
  goal: GoalId | null,
  guess: RuleAction[],
  words: { actions: RuleAction[] } | undefined,
): RuleAction[] {
  switch (goal) {
    case "guess": {
      return guess;
    }
    case "highlight": {
      return HIGHLIGHT;
    }
    case "newsletters": {
      return NEWSLETTERS;
    }
    case "read": {
      return MARK_READ;
    }
    case "words": {
      return words?.actions ?? [];
    }
    case null: {
      return [];
    }
  }
}

/** Whether a rule only shows a sender's images: it changes no mail. */
function showsImages(actions: readonly RuleAction[]): boolean {
  return actions.length > 0 && actions.every((action) => action.action === "show_images");
}

export interface SmartRuleConfirmProps {
  workspaceId: string;
  /** The rule to confirm; `null` while closed. */
  draft: SmartRuleDraft | null;
  /** Cancel (nothing saved), or once the result was shown. */
  onClose: () => void;
  /** Saved: the rule, its run started when it applies to the mail already here. */
  onSaved?: ((rule: RuleView) => void) | undefined;
}

/**
 * The one confirmation every offered rule goes through before anything is
 * saved (campbooks-cloud#169): the rule as a sentence labelled "Smart
 * rule", that it runs on its own with no AI and lives in Scout's memory,
 * what it matches (a count and a few subjects, as plain text), whether to
 * apply it to the mail already here, then Save smart rule (⌘↵) or Cancel
 * (Esc). Saving shows its result for about a second; the toast's Undo
 * removes the rule and puts back what it changed.
 */
export function SmartRuleConfirm({ workspaceId, draft, onClose, onSaved }: SmartRuleConfirmProps) {
  if (draft === null) {
    return null;
  }
  return (
    <Confirm
      // A new draft starts over (its own preview, the box checked).
      key={`${draft.inboxId}:${JSON.stringify(draft.conditions)}:${JSON.stringify(draft.actions)}`}
      workspaceId={workspaceId}
      draft={draft}
      onClose={onClose}
      onSaved={onSaved}
    />
  );
}

function Confirm({
  workspaceId,
  draft,
  onClose,
  onSaved,
}: SmartRuleConfirmProps & { draft: SmartRuleDraft }) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const locale = i18n.language;
  const undo = useUndoToast();
  const { create, remove, restore, wasRemoved } = useRuleMutations(workspaceId);
  const [choice, setChoice] = useState(draft.choices?.[0]?.id);
  const chosen = draft.choices?.find((each) => each.id === choice);
  const conditions = chosen?.conditions ?? draft.conditions;
  // "Scout noticed": what should happen comes first, and everything below
  // (the sentence, what it matches, the existing mail) follows the answer.
  const noticed = draft.goal;
  const [goal, setGoal] = useState<GoalId | null>(noticed?.start === "guess" ? "guess" : null);
  const [words, setWords] = useState<{ text: string; actions: RuleAction[] } | undefined>();
  const actions = noticed === undefined ? draft.actions : goalActions(goal, draft.actions, words);
  const ready = actions.length > 0;
  const images = showsImages(actions);
  const rule = { conditions, actions };
  const preview = useRulePreview(workspaceId, draft.inboxId, rule, { enabled: ready });
  const [existing, setExisting] = useState(true);
  const [failed, setFailed] = useState(false);
  // A rule already trusts this sender's images (paused or waiting too): the
  // server refuses a second one, and saving again would change nothing.
  const [already, setAlready] = useState(false);
  const [saved, setSaved] = useState(false);
  const sentence = ready ? ruleSentence(t, locale, rule) : "";
  const count = preview.data?.would_change ?? 0;
  const complete = preview.data?.complete ?? true;
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const day = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" });
  const samples: RulePreviewSample[] = (preview.data?.samples ?? [])
    .slice(0, EXAMPLES)
    .map((sample) => ({
      subject: sample.subject,
      sender: sample.sender,
      when: day.format(new Date(sample.sent_at)),
    }));
  let summary: string | undefined;
  if (!ready) {
    summary = undefined;
  } else if (preview.isError) {
    summary = t("smartRule.matchesUnknown");
  } else if (images && preview.data !== undefined) {
    // It changes no mail: say whose mail it opens up, not what it would change.
    const matching = preview.data.matching;
    summary =
      matching === 0
        ? t("smartRule.imagesNew")
        : t("smartRule.imagesMatches", { count: matching, n: number(matching) });
  } else if (preview.data !== undefined) {
    summary =
      count === 0
        ? t("smartRule.noMatches")
        : complete
          ? t("smartRule.matches", { count, n: number(count) })
          : t("smartRule.matchesAtLeast", { n: number(count) });
  }

  const save = () => {
    if (create.isPending || saved || already || !ready) {
      return;
    }
    setFailed(false);
    // A promise, not mutate's callbacks: closed while saving, the rule is
    // still made, so its Undo must still be offered.
    create
      .mutateAsync({
        inboxId: draft.inboxId,
        conditions,
        actions,
        applyToExisting: existing && count > 0 && !images,
        origin: draft.origin,
        fromConversation: draft.fromConversation,
        noticeId: draft.noticeId,
      })
      .then(
        ({ rule }) => {
          setSaved(true);
          onSaved?.(rule);
          undo.push({
            label: t("smartRule.toast", { rule: sentence }),
            // A promise, not mutate's callbacks: the dialog is gone by the
            // time Undo is pressed, and a failure must still be said.
            undo: () => {
              // Undo all on the run already removed it: never twice.
              if (wasRemoved(rule.id)) {
                return;
              }
              remove
                .mutateAsync({ ruleId: rule.id, restore: true })
                .then(() => {
                  // Undo of the Undo brings the same rule back, applied again.
                  undo.push({
                    label: t("rules.toast.stopped", { rule: sentence }),
                    undo: () => {
                      restore.mutateAsync({ ruleId: rule.id, applyToExisting: true }).catch(() => {
                        undo.notify(t("rules.toast.failed"));
                      });
                    },
                  });
                })
                .catch(() => {
                  undo.notify(t("rules.run.undoFailed"));
                });
            },
          });
        },
        (error: unknown) => {
          if (problemCode(error) === "images_already_trusted") {
            setAlready(true);
          } else {
            setFailed(true);
          }
        },
      );
  };
  useShortcut("inbox.rule.save", save, { enabled: !saved && !already && ready });

  return (
    <Dialog
      open
      onClose={onClose}
      title={chosen?.heading ?? draft.title}
      description={t("smartRule.lead")}
      result={
        saved ? { title: t("smartRule.saved"), detail: t("smartRule.savedDetail") } : undefined
      }
      onResultEnd={onClose}
      footer={
        <>
          <Button
            variant="primary"
            shortcut="inbox.rule.save"
            disabled={create.isPending || already || !ready}
            onClick={save}
          >
            {failed ? t("smartRule.retry") : t("smartRule.save")}
          </Button>
          <Button variant="ghost" shortcut="overlay.close" onClick={onClose}>
            {t("smartRule.cancel")}
          </Button>
        </>
      }
    >
      <div className="flex flex-col gap-3">
        {noticed === undefined ? null : (
          <Goal
            workspaceId={workspaceId}
            draft={draft}
            goal={goal}
            onGoal={setGoal}
            words={words}
            onWords={(found) => {
              setWords(found);
              setGoal("words");
            }}
          />
        )}
        {draft.choices === undefined || draft.choices.length < 2 ? null : (
          <ChoiceCards
            label={t("smartRule.whose")}
            options={draft.choices.map((each) => ({
              value: each.id,
              title: each.title,
              description: each.description,
            }))}
            value={choice ?? null}
            onChange={setChoice}
          />
        )}
        {ready ? (
          <RulePreview
            tag={t("smartRule.tag")}
            sentence={sentence}
            summary={summary}
            samples={images ? [] : samples}
            samplesLabel={t("smartRule.examples")}
            pending={preview.isPending}
          />
        ) : (
          <Text tone="secondary" size="sm">
            {t("smartRule.goal.pick")}
          </Text>
        )}
        <Text tone="secondary" size="sm">
          {t("smartRule.how")}
        </Text>
        {images ? (
          <Text tone="secondary" size="sm">
            {t("smartRule.imagesHow")}
          </Text>
        ) : null}
        {draft.sharedInbox === undefined ? null : (
          <Text tone="secondary" size="sm">
            {t("smartRule.shared", { inbox: draft.sharedInbox })}
          </Text>
        )}
        {ready && count > 0 && !images ? (
          <Checkbox
            label={t("smartRule.existing", { count, n: number(count) })}
            checked={existing}
            onChange={(event) => {
              setExisting(event.target.checked);
            }}
          />
        ) : null}
        {failed ? <Banner tone="danger" role="alert" title={t("smartRule.failed")} /> : null}
        {already ? <Banner role="status" title={t("smartRule.imagesAlready")} /> : null}
      </div>
    </Dialog>
  );
}

/**
 * "What should happen to <sender>'s mail?" (campbooks-cloud#274): Scout's
 * guess, "Highlight them", filing under Newsletters, marking read, or the
 * member's own words (read by Scout through the same instruction path Scout's
 * memory uses; only the actions it finds count, the conditions stay the
 * sender's).
 */
function Goal({
  workspaceId,
  draft,
  goal,
  onGoal,
  words,
  onWords,
}: {
  workspaceId: string;
  draft: SmartRuleDraft;
  goal: GoalId | null;
  onGoal: (goal: GoalId) => void;
  words: { text: string; actions: RuleAction[] } | undefined;
  onWords: (found: { text: string; actions: RuleAction[] }) => void;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const compile = useCompileGoal(workspaceId);
  const [text, setText] = useState("");
  const [unclear, setUnclear] = useState(false);
  const kind = draft.goal?.kind ?? "archive_on_arrival";
  const stream = streamName(t, ruleStream(draft.actions) ?? "updates");
  const newsletters = streamName(t, "newsletters");
  const options: ChoiceCardOption<GoalId>[] = [
    kind === "archive_on_arrival"
      ? {
          value: "guess",
          title: t("smartRule.goal.archive"),
          description: t("smartRule.goal.archiveDetail"),
          tag: t("smartRule.goal.guess"),
        }
      : {
          value: "guess",
          title: t("smartRule.goal.fileOffHome", { stream }),
          description: t("smartRule.goal.fileOffHomeDetail", { stream }),
          tag: t("smartRule.goal.guess"),
        },
    {
      value: "highlight",
      title: t("smartRule.goal.highlight"),
      description: t("smartRule.goal.highlightDetail"),
      tag: t("smartRule.goal.highlightTag"),
    },
    // Scout's guess may already be this one.
    ...(sameActions(NEWSLETTERS, draft.actions)
      ? []
      : [
          {
            value: "newsletters" as const,
            title: t("smartRule.goal.newsletters", { stream: newsletters }),
            description: t("smartRule.goal.newslettersDetail"),
          },
        ]),
    {
      value: "read",
      title: t("smartRule.goal.read"),
      description: t("smartRule.goal.readDetail"),
    },
    ...(words === undefined
      ? []
      : [{ value: "words" as const, title: t("smartRule.goal.words"), description: words.text }]),
  ];
  const ask = () => {
    const sentence = text.trim();
    if (sentence === "" || compile.isPending) {
      return;
    }
    setUnclear(false);
    compile.mutate(`${draft.goal?.sender ?? ""}: ${sentence}`, {
      onSuccess: (result) => {
        const found = result.proposals[0];
        if (result.outcome !== "rules" || found === undefined || found.actions.length === 0) {
          setUnclear(true);
          return;
        }
        onWords({ text: sentence, actions: found.actions });
      },
    });
  };
  const code = compile.isError ? problemCode(compile.error) : undefined;
  let problem: string | undefined;
  if (unclear) {
    problem = t("smartRule.goal.wordsUnclear");
  } else if (compile.isError) {
    if (code === "compile_unavailable") {
      problem = t("smartRule.goal.wordsNoModel");
    } else if (errorStatus(compile.error) === 429) {
      problem = t("smartRule.goal.wordsBusy");
    } else {
      problem = t("smartRule.goal.wordsFailed");
    }
  }
  return (
    <div className="flex flex-col gap-3">
      <ChoiceCards
        label={t("smartRule.goal.question", { sender: draft.goal?.sender ?? "" })}
        options={options}
        value={goal}
        onChange={onGoal}
      />
      <form
        className="flex flex-col gap-2 sm:flex-row sm:items-end"
        aria-label={t("smartRule.goal.wordsLabel")}
        onSubmit={(event) => {
          event.preventDefault();
          ask();
        }}
      >
        <div className="min-w-0 flex-1">
          <TextField
            label={t("smartRule.goal.wordsLabel")}
            placeholder={t("smartRule.goal.wordsPlaceholder")}
            maxLength={300}
            value={text}
            onChange={(event) => {
              setText(event.target.value);
            }}
          />
        </div>
        <Button type="submit" ai disabled={compile.isPending || text.trim() === ""}>
          {t("smartRule.goal.wordsAsk")}
        </Button>
      </form>
      {problem === undefined ? null : <Banner tone="danger" role="alert" title={problem} />}
    </div>
  );
}

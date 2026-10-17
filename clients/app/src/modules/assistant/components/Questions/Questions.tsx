import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useShortcut } from "~/lib/shortcuts";
import {
  Banner,
  Brand,
  Button,
  CheckCards,
  ChoiceCards,
  FocusPage,
  Heading,
  SkeletonList,
  Text,
  useUndoToast,
  Visor,
} from "~/lib/ui";
import { useInboxes } from "~/modules/conversations";

import {
  useCreateRules,
  useProfile,
  useRemoveRules,
  useSenders,
  useUpdateProfile,
  type NewRule,
  type Proactivity,
  type Sender,
} from "../../api/queries";
import { answerRules, teachable, teachingId, type Answers, type TeachInbox } from "../../model";
import { RuleMatch } from "../RuleMatch/RuleMatch";
import { SenderFilter } from "../SenderFilter/SenderFilter";

/** The questions, then the review and what was saved. */
type Step = "always" | "never" | "act" | "review" | "done";
const QUESTIONS = ["always", "never", "act"] as const;

export interface QuestionsProps {
  workspaceId: string;
  /** On to the workspace's Home (Not now, and after saving). */
  onHome: () => void;
  /** On to Scout's memory. */
  onMemory: () => void;
}

/**
 * "Tell Scout what you care about": three questions, one per view, every
 * one skippable (S), Not now (Esc) anytime. Each answer shows the rule it
 * becomes with what it matches; the review saves them all at once, with
 * one Undo.
 */
export function Questions({ workspaceId, onHome, onMemory }: QuestionsProps) {
  const { t } = useTranslation("assistant");
  const { t: tShell } = useTranslation("shell");
  const inboxes = useInboxes(workspaceId);
  const profile = useProfile(workspaceId);
  const update = useUpdateProfile(workspaceId);
  const create = useCreateRules(workspaceId);
  const remove = useRemoveRules(workspaceId);
  const undo = useUndoToast();
  const senders = useSenders(workspaceId);

  const [step, setStep] = useState<Step>("always");
  const [answers, setAnswers] = useState<Answers>({ always: [], never: [], neverSenders: [] });
  // Senders found in the filter box and ticked, so their names stay.
  const [found, setFound] = useState<ReadonlyMap<string, Sender>>(new Map());
  const [chosen, setChosen] = useState<Proactivity | undefined>();
  const [sharedOn, setSharedOn] = useState<string[]>([]);
  const [existing, setExisting] = useState(true);
  // Rules created so far, by `<rule>|<inbox>`, so a retry sends only the rest.
  const [created, setCreated] = useState<ReadonlyMap<string, string>>(new Map());
  // One teaching per answer, the same across retries.
  const [teachings] = useState(() => new Map<string, string>());
  const [failure, setFailure] = useState<"rules" | "answers" | undefined>();
  // What saving changed, for the done screen.
  const [saved, setSaved] = useState<{ rules: number; proactivity: Proactivity | undefined }>({
    rules: 0,
    proactivity: undefined,
  });
  const proactivity = chosen ?? profile.data?.proactivity ?? "ask_first";

  const all: TeachInbox[] = (inboxes.data ?? []).map((inbox) => ({
    id: inbox.id,
    address: inbox.address,
    role: inbox.role,
    shared: inbox.shared,
  }));
  const mine = all.filter((inbox) => teachable(inbox) && !inbox.shared);
  const shared = all.filter((inbox) => teachable(inbox) && inbox.shared);
  const readOnly = all.filter((inbox) => !teachable(inbox));
  const targets = [...mine, ...shared.filter((inbox) => sharedOn.includes(inbox.id))];
  const known = new Map<string, Sender>([
    ...(senders.data ?? []).map((sender) => [sender.address, sender] as const),
    ...found,
  ]);
  const remember = (sender: Sender | undefined) => {
    if (sender !== undefined) {
      setFound((current) => new Map(current).set(sender.address, sender));
    }
  };
  const names = new Map(
    [...known.values()].flatMap((sender) =>
      sender.name === null || sender.name === undefined
        ? []
        : [[sender.address, sender.name] as const],
    ),
  );
  const rules = answerRules(answers);
  const asking = (QUESTIONS as readonly Step[]).includes(step);
  const position = QUESTIONS.indexOf(step as (typeof QUESTIONS)[number]) + 1;

  const notNow = () => {
    // Not while saving: the save's own update and Undo come first.
    if (create.isPending || update.isPending) {
      return;
    }
    void (async () => {
      // Wait for the answers rather than overwrite them; when they can't be
      // read, nothing is written and the flow stays, saying so.
      const current = profile.data ?? (await profile.refetch()).data;
      if (current === undefined) {
        undo.notify(t("flow.notNowFailed"));
        return;
      }
      update.mutate(
        { proactivity: current.proactivity, asked: true },
        {
          onSuccess: onHome,
          onError: () => {
            undo.notify(t("flow.notNowFailed"));
          },
        },
      );
    })();
  };
  const next = () => {
    setStep((current) => {
      switch (current) {
        case "always": {
          return "never";
        }
        case "never": {
          return "act";
        }
        default: {
          return "review";
        }
      }
    });
  };
  const back = () => {
    setStep((current) => (current === "review" ? "act" : current === "act" ? "never" : "always"));
  };
  const skip = () => {
    if (step === "always") {
      setAnswers((a) => ({ ...a, always: [] }));
    } else if (step === "never") {
      setAnswers((a) => ({ ...a, never: [], neverSenders: [] }));
    } else {
      setChosen(undefined);
    }
    next();
  };
  useShortcut("scout.skip", skip, { enabled: asking });
  useShortcut("scout.back", back, { enabled: step !== "always" && step !== "done" });
  useShortcut("overlay.close", notNow, { enabled: step !== "done" });

  // Every rule in every chosen inbox; the ones not created yet.
  const wanted: NewRule[] = rules.flatMap((rule) => {
    let teaching = teachings.get(rule.key);
    if (teaching === undefined) {
      teaching = teachingId();
      teachings.set(rule.key, teaching);
    }
    const id = teaching;
    return targets.map((inbox) => ({
      key: `${rule.key}|${inbox.id}`,
      inboxId: inbox.id,
      conditions: rule.conditions,
      actions: rule.actions,
      origin: "onboarding" as const,
      applyToExisting: existing,
      teachingId: id,
    }));
  });
  const pending = wanted.filter((rule) => !created.has(rule.key));
  // Rules with nowhere to go: Save waits for an inbox (once they're known).
  const noTarget = rules.length > 0 && inboxes.isSuccess && targets.length === 0;
  const inboxesUnknown = rules.length > 0 && !inboxes.isSuccess;
  // Saving writes the answer about acting: never over one it couldn't read.
  const profileUnknown = profile.data === undefined;
  const busy = create.isPending || update.isPending;

  /** The answers, once every rule is in: then the done screen and one Undo. */
  const finish = (all: ReadonlyMap<string, string>) => {
    const previous = profile.data;
    update.mutate(
      { proactivity, asked: true },
      {
        onError: () => {
          setFailure("answers");
        },
        onSuccess: () => {
          setFailure(undefined);
          const ids = [...all.values()];
          // Rules that exist somewhere, not the answers given.
          const count = new Set([...all.keys()].map((key) => key.split("|", 1)[0])).size;
          setSaved({
            rules: count,
            proactivity: previous?.proactivity === proactivity ? undefined : proactivity,
          });
          setStep("done");
          undo.push({
            label: count === 0 ? t("flow.done.toastAnswers") : t("flow.done.toast", { count }),
            // Promises, not mutate's callbacks: Undo may come after leaving
            // for Home, and a failure must still be said.
            undo: () => {
              Promise.all([
                ids.length > 0 ? remove.mutateAsync(ids) : undefined,
                update.mutateAsync({
                  proactivity: previous?.proactivity ?? "ask_first",
                  asked: true,
                }),
              ]).catch(() => {
                undo.notify(t("flow.undoFailed"));
              });
            },
          });
        },
      },
    );
  };
  const save = () => {
    if (busy || noTarget || inboxesUnknown || profileUnknown) {
      return;
    }
    if (pending.length === 0) {
      finish(created);
      return;
    }
    create.mutate(pending, {
      onSuccess: (result) => {
        const all = new Map(created);
        for (const each of result.created) {
          all.set(each.key, each.id);
        }
        setCreated(all);
        if (result.failed) {
          setFailure("rules");
          return;
        }
        finish(all);
      },
      onError: () => {
        setFailure("rules");
      },
    });
  };

  // Enter on a ticked checkbox moves on too; a focused button or link
  // handles its own Enter.
  useShortcut("scout.next", (event) => {
    const target = event?.target;
    if (target instanceof HTMLButtonElement || target instanceof HTMLAnchorElement) {
      return;
    }
    if (asking) {
      next();
    } else if (step === "review") {
      save();
    } else {
      onHome();
    }
  });

  const top = (
    <>
      <Brand name={tShell("appName")} />
      {step === "done" ? null : (
        <Button variant="ghost" shortcut="overlay.close" onClick={notNow}>
          {t("flow.notNow")}
        </Button>
      )}
    </>
  );
  const questionActions = (
    <>
      <Button type="submit" variant="primary" size="lg" shortcut="scout.next">
        {step === "act" ? t("flow.review") : t("flow.next")}
      </Button>
      <Button variant="ghost" size="lg" shortcut="scout.skip" onClick={skip}>
        {t("flow.skip")}
      </Button>
      {step === "always" ? null : (
        <Button variant="ghost" size="lg" shortcut="scout.back" onClick={back}>
          {t("flow.back")}
        </Button>
      )}
    </>
  );
  const becomes = (keys: string[]) => {
    const shown = rules.filter((rule) => keys.includes(rule.key));
    return shown.length === 0 ? null : (
      <section className="flex flex-col gap-2" aria-label={t("flow.becomes")}>
        <Heading level={2} size="section">
          {t("flow.becomes")}
        </Heading>
        {shown.map((rule) => (
          <RuleMatch
            key={rule.key}
            workspaceId={workspaceId}
            rule={rule}
            inboxIds={mine.map((inbox) => inbox.id)}
            names={names}
          />
        ))}
      </section>
    );
  };

  switch (step) {
    case "always": {
      return (
        <FocusPage
          top={top}
          eyebrow={t("flow.step", { step: position, total: QUESTIONS.length })}
          title={t("flow.always.title")}
          lead={t("flow.always.lead")}
          footer={t("flow.footer")}
          onSubmit={next}
          actions={questionActions}
        >
          <SenderFilter
            workspaceId={workspaceId}
            label={t("flow.always.label")}
            suggest
            known={known}
            value={answers.always}
            onChange={(always, ticked) => {
              remember(ticked);
              setAnswers((a) => ({ ...a, always }));
            }}
          />
          {becomes(answers.always.map((sender) => `always:${sender}`))}
        </FocusPage>
      );
    }
    case "never": {
      return (
        <FocusPage
          top={top}
          eyebrow={t("flow.step", { step: position, total: QUESTIONS.length })}
          title={t("flow.never.title")}
          lead={t("flow.never.lead")}
          footer={t("flow.footer")}
          onSubmit={next}
          actions={questionActions}
        >
          <CheckCards
            label={t("flow.never.label")}
            hideLabel
            value={answers.never}
            onChange={(never) => {
              setAnswers((a) => ({ ...a, never }));
            }}
            options={[
              {
                value: "lists" as const,
                title: t("flow.never.lists"),
                description: t("flow.never.listsHint"),
              },
              {
                value: "automatic" as const,
                title: t("flow.never.automatic"),
                description: t("flow.never.automaticHint"),
              },
            ]}
          />
          <SenderFilter
            workspaceId={workspaceId}
            label={t("flow.never.senders")}
            known={known}
            value={answers.neverSenders}
            onChange={(neverSenders, ticked) => {
              remember(ticked);
              setAnswers((a) => ({ ...a, neverSenders }));
            }}
          />
          {becomes([
            ...answers.never.map((kind) => `never:${kind}`),
            ...answers.neverSenders.map((sender) => `never:${sender}`),
          ])}
        </FocusPage>
      );
    }
    case "act": {
      return (
        <FocusPage
          top={top}
          eyebrow={t("flow.step", { step: position, total: QUESTIONS.length })}
          title={t("flow.act.title")}
          lead={t("flow.act.lead")}
          footer={t("flow.footer")}
          onSubmit={next}
          actions={questionActions}
        >
          <ChoiceCards
            label={t("flow.act.label")}
            hideLabel
            value={proactivity}
            onChange={setChosen}
            options={[
              {
                value: "ask_first",
                title: t("flow.act.askFirst"),
                description: t("flow.act.askFirstHint"),
              },
              {
                value: "act_then_tell",
                title: t("flow.act.actThenTell"),
                description: t("flow.act.actThenTellHint"),
              },
            ]}
          />
        </FocusPage>
      );
    }
    case "review": {
      const createdCount = wanted.length - pending.length;
      return (
        <FocusPage
          top={top}
          title={t("flow.reviewStep.title")}
          lead={
            rules.length === 0
              ? t("flow.reviewStep.none")
              : t("flow.reviewStep.rules", { count: rules.length })
          }
          footer={t("flow.footer")}
          onSubmit={save}
          actions={
            <>
              <Button
                type="submit"
                variant="primary"
                size="lg"
                shortcut="scout.next"
                disabled={busy || noTarget || inboxesUnknown || profileUnknown}
              >
                {busy
                  ? t("flow.reviewStep.saving")
                  : failure === undefined
                    ? t("flow.reviewStep.save")
                    : t("flow.reviewStep.retry")}
              </Button>
              <Button
                variant="ghost"
                size="lg"
                shortcut="scout.back"
                disabled={busy}
                onClick={back}
              >
                {t("flow.back")}
              </Button>
            </>
          }
        >
          {rules.map((rule) => (
            <RuleMatch
              key={rule.key}
              workspaceId={workspaceId}
              rule={rule}
              inboxIds={targets.map((inbox) => inbox.id)}
              names={names}
            />
          ))}
          {rules.length > 0 && inboxes.isPending ? <SkeletonList rows={1} /> : null}
          {rules.length > 0 && inboxes.isError ? (
            <Banner
              tone="danger"
              role="alert"
              title={t("flow.reviewStep.inboxesFailed")}
              action={
                <Button
                  variant="secondary"
                  onClick={() => {
                    void inboxes.refetch();
                  }}
                >
                  {t("flow.reviewStep.tryAgain")}
                </Button>
              }
            />
          ) : null}
          {rules.length === 0 || !inboxes.isSuccess ? null : (
            <section className="flex flex-col gap-2" aria-label={t("flow.reviewStep.where")}>
              <Heading level={2} size="section">
                {t("flow.reviewStep.where")}
              </Heading>
              {mine.map((inbox) => (
                <Text key={inbox.id}>
                  {inbox.address} · {t("flow.reviewStep.yours")}
                </Text>
              ))}
              {shared.length === 0 ? null : (
                <CheckCards
                  label={t("flow.reviewStep.where")}
                  hideLabel
                  value={sharedOn}
                  onChange={setSharedOn}
                  options={shared.map((inbox) => ({
                    value: inbox.id,
                    title: inbox.address,
                    description: t("flow.reviewStep.shared"),
                  }))}
                />
              )}
              {readOnly.map((inbox) => (
                <Text key={inbox.id} tone="tertiary" size="sm">
                  {t("flow.reviewStep.readOnly", { address: inbox.address })}
                </Text>
              ))}
              <CheckCards
                label={t("flow.reviewStep.existing")}
                hideLabel
                value={existing ? ["existing"] : []}
                onChange={(values) => {
                  setExisting(values.includes("existing"));
                }}
                options={[
                  {
                    value: "existing",
                    title: t("flow.reviewStep.existing"),
                    description: t("flow.reviewStep.existingHint"),
                  },
                ]}
              />
            </section>
          )}
          <Text tone="secondary">
            {proactivity === "ask_first"
              ? t("flow.reviewStep.askFirst")
              : t("flow.reviewStep.actThenTell")}
          </Text>
          {profile.isError ? (
            <Banner
              tone="danger"
              role="alert"
              title={t("memory.questions.failed")}
              action={
                <Button
                  variant="secondary"
                  onClick={() => {
                    void profile.refetch();
                  }}
                >
                  {t("flow.reviewStep.tryAgain")}
                </Button>
              }
            />
          ) : null}
          {noTarget ? (
            <Banner
              role="status"
              title={
                shared.length > 0 ? t("flow.reviewStep.chooseInbox") : t("flow.reviewStep.nowhere")
              }
            />
          ) : null}
          {failure === "rules" ? (
            <Banner
              tone="danger"
              role="alert"
              title={
                createdCount === 0
                  ? t("flow.reviewStep.rulesFailed")
                  : t("flow.reviewStep.partial", { saved: createdCount, total: wanted.length })
              }
            />
          ) : null}
          {failure === "answers" ? (
            <Banner tone="danger" role="alert" title={t("flow.reviewStep.answersFailed")} />
          ) : null}
        </FocusPage>
      );
    }
    case "done": {
      return (
        <FocusPage
          top={top}
          leading={<Visor state="happy" size="xl" />}
          title={t("flow.done.title")}
          lead={
            saved.rules > 0
              ? t("flow.done.lead", { count: saved.rules })
              : saved.proactivity === "act_then_tell"
                ? t("flow.done.leadActThenTell")
                : saved.proactivity === "ask_first"
                  ? t("flow.done.leadAskFirst")
                  : t("flow.done.leadSame")
          }
          onSubmit={onHome}
          actions={
            <>
              <Button type="submit" variant="primary" size="lg" shortcut="scout.next">
                {t("flow.done.home")}
              </Button>
              <Button variant="ghost" size="lg" onClick={onMemory}>
                {t("flow.done.memory")}
              </Button>
            </>
          }
        />
      );
    }
  }
}

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { Banner, Button, Note, PrivacyLine, Text, TextField } from "~/lib/ui";

import { useCompile, type CompileResult, type Proposal, type RuleShape } from "../../api/queries";
import { answerWho, type Translate } from "../../model";
import { AddSender } from "../AddSender/AddSender";
import { RuleBuilder } from "../RuleBuilder/RuleBuilder";
import { RuleSave } from "../RuleSave/RuleSave";

/** Longest sentence, as the server takes it. */
const MAX_CHARS = 300;

/** Why the server refuses a sentence (`assistant.compile_instruction`, 400). */
const REFUSALS = new Set([
  "instruction_empty",
  "instruction_too_long",
  "instruction_has_link",
  "instruction_pasted_mail",
]);

export interface TeachScoutProps {
  workspaceId: string;
  /**
   * A sentence to start from (Scout proposed it in a chat): it is in the
   * field and read at once, then checked and saved as any other.
   */
  initial?: string | undefined;
  /** A rule was saved, by the member's own Save. */
  onSaved?: (() => void) | undefined;
}

/** Why the builder is shown instead of the sentence. */
type BuilderReason = "chosen" | "failed";

/**
 * "Teach Scout something": the member's sentence becomes rules to check
 * (`assistant.compile_instruction`, only when they press Enter). Scout asks
 * who someone is rather than guessing, says plainly what rules can't do, and
 * shows each rule with what it matches before Save. "Build a rule yourself"
 * opens a rule builder at any time, with no model call; without a language
 * model here, the builder is all there is.
 */
export function TeachScout({ workspaceId, initial, onSaved }: TeachScoutProps) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const compile = useCompile(workspaceId);
  const [text, setText] = useState(initial ?? "");
  const [result, setResult] = useState<CompileResult | undefined>();
  const [builder, setBuilder] = useState<BuilderReason | undefined>();
  // A sentence handed over is read once, as if the member pressed Teach.
  const started = useRef(false);
  const { mutate } = compile;
  useEffect(() => {
    if (started.current || initial === undefined || initial.trim() === "") {
      return;
    }
    started.current = true;
    mutate(initial, { onSuccess: setResult });
  }, [initial, mutate]);

  const code = compile.isError ? problemCode(compile.error) : undefined;
  const noModel = code === "compile_unavailable";
  const reset = () => {
    setResult(undefined);
    compile.reset();
  };
  const build = () => {
    setBuilder("chosen");
  };
  if (builder !== undefined || noModel) {
    const lead = noModel
      ? t("memory.builder.noModel")
      : builder === "failed"
        ? t("memory.builder.byHand")
        : t("memory.builder.lead");
    return (
      <div className="flex flex-col gap-3">
        <Note visor={noModel || builder === "failed" ? "unsure" : "idle"} label={lead}>
          {lead}
        </Note>
        <RuleBuilder workspaceId={workspaceId} onSaved={onSaved} />
        {noModel ? null : (
          <div className="flex flex-wrap gap-2">
            <Button
              variant="ghost"
              onClick={() => {
                setBuilder(undefined);
                reset();
              }}
            >
              {t("memory.builder.back")}
            </Button>
          </div>
        )}
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-3">
      <form
        className="flex flex-col gap-2 sm:flex-row sm:items-end"
        aria-label={t("memory.teach.title")}
        onSubmit={(event) => {
          event.preventDefault();
          if (text.trim().length === 0 || compile.isPending) {
            return;
          }
          setResult(undefined);
          compile.mutate(text, { onSuccess: setResult });
        }}
      >
        <div className="min-w-0 flex-1">
          <TextField
            label={t("memory.teach.label")}
            hideLabel
            placeholder={t("memory.teach.placeholder")}
            maxLength={MAX_CHARS}
            value={text}
            onChange={(event) => {
              setText(event.target.value);
            }}
          />
        </div>
        <Button type="submit" variant="primary" keys="enter" disabled={compile.isPending}>
          {t("memory.teach.submit")}
        </Button>
      </form>
      <div className="flex flex-col items-start gap-2 sm:flex-row sm:items-center sm:justify-between">
        <PrivacyLine>{t("memory.teach.privacy")}</PrivacyLine>
        <Button variant="ghost" onClick={build}>
          {t("memory.builder.open")}
        </Button>
      </div>
      {compile.isPending ? (
        <Note visor="thinking" label={t("memory.teach.reading")}>
          <strong>{t("memory.teach.reading")}</strong> {t("memory.teach.readingLead")}
        </Note>
      ) : null}
      {compile.isError ? (
        <Banner
          tone="danger"
          role="alert"
          title={
            errorStatus(compile.error) === 429
              ? t("memory.teach.busy")
              : code !== undefined && REFUSALS.has(code)
                ? t(`memory.teach.refused.${code}`)
                : t("memory.teach.failed")
          }
          action={
            <Button
              variant="secondary"
              onClick={() => {
                setBuilder("failed");
              }}
            >
              {t("memory.builder.show")}
            </Button>
          }
        />
      ) : null}
      {result === undefined ? null : (
        <Understood
          workspaceId={workspaceId}
          result={result}
          onDone={reset}
          onBuild={build}
          onSaved={onSaved}
        />
      )}
    </div>
  );
}

/**
 * What Scout understood: the rules, or, when it couldn't make one or the
 * sentence asks for what rules can't do, why, with the builder one click
 * away.
 */
function Understood({
  workspaceId,
  result,
  onDone,
  onBuild,
  onSaved,
}: {
  workspaceId: string;
  result: CompileResult;
  onDone: () => void;
  /** Open the rule builder instead. */
  onBuild: () => void;
  onSaved?: (() => void) | undefined;
}) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const quote = { text: result.text, attribution: t("memory.teach.youWrote") };
  const proposals = result.proposals.map((proposal, index) => (
    <ProposalCard
      key={`${String(index)}-${JSON.stringify(proposal.conditions)}`}
      workspaceId={workspaceId}
      proposal={proposal}
      text={result.text}
      first={index === 0}
      onDone={onDone}
      onSaved={onSaved}
    />
  ));
  if (result.outcome === "rules") {
    return (
      <div className="flex flex-col gap-3">
        <Note visor="happy" quote={quote} label={t("memory.teach.understood")}>
          {proposals.length > 1 ? t("memory.teach.understoodMany") : t("memory.teach.understood")}
        </Note>
        {proposals}
      </div>
    );
  }
  const message =
    result.outcome === "unsupported" &&
    result.unsupported !== null &&
    result.unsupported !== undefined
      ? t(`memory.teach.unsupported.${result.unsupported}`)
      : t("memory.teach.unclear");
  return (
    <div className="flex flex-col gap-3">
      <Note
        visor="unsure"
        quote={quote}
        label={message}
        actions={
          <Button variant="secondary" onClick={onBuild}>
            {t("memory.builder.open")}
          </Button>
        }
      >
        {message}
      </Note>
      {proposals.length === 0 ? null : (
        <>
          <Text tone="secondary">{t("memory.teach.closest")}</Text>
          {proposals}
        </>
      )}
    </div>
  );
}

function ProposalCard({
  workspaceId,
  proposal,
  text,
  first,
  onDone,
  onSaved,
}: {
  workspaceId: string;
  proposal: Proposal;
  text: string;
  first: boolean;
  onDone: () => void;
  onSaved?: (() => void) | undefined;
}) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const [sender, setSender] = useState<string | undefined>();
  const who = proposal.who;
  if (who !== null && who !== undefined && sender === undefined) {
    return (
      <Note
        visor="unsure"
        label={t("memory.teach.who", { phrase: who.phrase })}
        choice={{
          prompt: who.matched ? t("memory.teach.whoLead") : t("memory.teach.whoNoMatch"),
          options: who.candidates.map((candidate) => ({
            label:
              candidate.name === null || candidate.name === undefined
                ? candidate.address
                : `${candidate.name} · ${candidate.address}`,
            onSelect: () => {
              setSender(candidate.address);
            },
          })),
        }}
        footer={<AddSender label={t("memory.teach.whoOther")} onAdd={setSender} />}
      >
        <strong>{t("memory.teach.who", { phrase: who.phrase })}</strong>
      </Note>
    );
  }
  const rule: RuleShape =
    sender === undefined
      ? { conditions: proposal.conditions, actions: proposal.actions }
      : answerWho(proposal, sender);
  const names = new Map(
    (who?.candidates ?? []).flatMap((candidate) =>
      candidate.name === null || candidate.name === undefined
        ? []
        : [[candidate.address, candidate.name] as const],
    ),
  );
  return (
    <RuleSave
      workspaceId={workspaceId}
      rule={rule}
      inboxes={proposal.inboxes}
      origin="instruction"
      originText={text}
      names={names}
      primary={first}
      onDone={onDone}
      onSaved={onSaved}
    />
  );
}

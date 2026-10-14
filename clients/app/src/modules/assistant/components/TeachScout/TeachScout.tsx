import { useState } from "react";
import { useTranslation } from "react-i18next";

import { errorStatus, problemCode } from "~/lib/api";
import { Banner, Button, Note, Text, TextField } from "~/lib/ui";

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
}

/**
 * "Teach Scout something": the member's sentence becomes rules to check
 * (`assistant.compile_instruction`, only when they press Enter). Scout asks
 * who someone is rather than guessing, says plainly what rules can't do, and
 * shows each rule with what it matches before Save. Without a language model
 * here, a rule builder instead.
 */
export function TeachScout({ workspaceId }: TeachScoutProps) {
  const { t: typed } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const compile = useCompile(workspaceId);
  const [text, setText] = useState("");
  const [result, setResult] = useState<CompileResult | undefined>();
  const [manual, setManual] = useState(false);

  const code = compile.isError ? problemCode(compile.error) : undefined;
  const noModel = code === "ai_role_not_configured" || code === "ai_external_content_refused";
  const reset = () => {
    setResult(undefined);
    compile.reset();
  };
  if (manual || noModel) {
    return (
      <div className="flex flex-col gap-3">
        <Note visor="unsure" label={t("memory.builder.noModel")}>
          {noModel ? t("memory.builder.noModel") : t("memory.teach.failed")}
        </Note>
        <RuleBuilder workspaceId={workspaceId} />
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
            hint={t("memory.teach.privacy")}
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
                setManual(true);
              }}
            >
              {t("memory.builder.show")}
            </Button>
          }
        />
      ) : null}
      {result === undefined ? null : (
        <Understood workspaceId={workspaceId} result={result} onDone={reset} />
      )}
    </div>
  );
}

function Understood({
  workspaceId,
  result,
  onDone,
}: {
  workspaceId: string;
  result: CompileResult;
  onDone: () => void;
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
      <Note visor="unsure" quote={quote} label={message}>
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
}: {
  workspaceId: string;
  proposal: Proposal;
  text: string;
  first: boolean;
  onDone: () => void;
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
      focusSave={first}
      onDone={onDone}
    />
  );
}

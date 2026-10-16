import { useState } from "react";
import { useTranslation } from "react-i18next";

import { useSession } from "~/lib/api";
import {
  Button,
  ChoiceCards,
  EmptyState,
  Heading,
  InlineConfirm,
  List,
  ListRow,
  SkeletonList,
} from "~/lib/ui";
import { useMembers } from "~/modules/workspaces";

import { useInboxes } from "../../api/queries";
import { useRules, type RuleView } from "../../api/rules";
import { listTime, type Translate } from "../../model";
import { originLine, ruleSentence, type RuleAuthor } from "../../rules-text";
import { useRuleActions } from "../TeachOnce/use-rule-actions";

/**
 * Who taught each rule: you, a named teammate, or someone the list cannot
 * name; `undefined` while that is not known yet (the members or the session
 * still loading), so a rule never reads as someone else's for a moment.
 */
function useAuthors(workspaceId: string): (rule: RuleView) => RuleAuthor | undefined {
  const members = useMembers(workspaceId);
  const session = useSession();
  const myEmail = session.data?.user?.email?.toLowerCase();
  const loading = members.isPending || session.isPending;
  return (rule) => {
    if (loading) {
      return;
    }
    const member = members.data?.find((item) => item.user_id === rule.taught_by);
    if (member === undefined) {
      return { kind: "someone" };
    }
    if (myEmail !== undefined && member.email?.toLowerCase() === myEmail) {
      return { kind: "you" };
    }
    const name = member.display_name ?? member.email ?? "";
    return name === "" ? { kind: "someone" } : { kind: "named", name };
  };
}

/**
 * The taught rules of the inboxes the member can see, as sentences: the ones
 * waiting for a yes first (Keep this rule / No), then each inbox's rules with
 * who taught them, how often and when they last acted, and Remove (optionally
 * putting back what they changed). Every change says so, with Undo. The
 * rules slot of Scout's memory.
 * @public
 */
export function RulesList({ workspaceId }: { workspaceId: string }) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const rules = useRules(workspaceId);
  const inboxes = useInboxes(workspaceId);
  const authorOf = useAuthors(workspaceId);
  const locale = i18n.language;

  if (rules.isError) {
    return (
      <EmptyState
        title={t("rules.list.failed")}
        visor="thinking"
        headingLevel={2}
        action={
          <Button
            variant="secondary"
            onClick={() => {
              void rules.refetch();
            }}
          >
            {t("rules.list.retry")}
          </Button>
        }
      />
    );
  }
  if (rules.data === undefined) {
    return <SkeletonList rows={3} />;
  }
  if (rules.data.length === 0) {
    return (
      <EmptyState title={t("rules.list.label")} visor="idle" headingLevel={2}>
        {t("rules.list.empty")}
      </EmptyState>
    );
  }
  // Keep / No act on the inbox (anyone who may teach there): only offered
  // where the member writes, never in a reader's or a kept inbox.
  const teaches = (rule: RuleView) => {
    const inbox = inboxes.data?.find((item) => item.id === rule.inbox_id);
    return (
      inbox !== undefined &&
      !inbox.read_only &&
      (inbox.role === "sender" || inbox.role === "manager")
    );
  };
  const row = (rule: RuleView) => (
    <RuleRow
      key={rule.id}
      workspaceId={workspaceId}
      rule={rule}
      locale={locale}
      author={authorOf(rule)}
      teaches={teaches(rule)}
    />
  );
  const waiting = rules.data.filter((rule) => rule.state === "proposed");
  // Grouped by the rules' own inboxes, so they show while the inbox list
  // loads, or if it cannot.
  const inboxIds = [
    ...new Set(rules.data.filter((rule) => rule.state !== "proposed").map((r) => r.inbox_id)),
  ];
  return (
    <div className="flex flex-col gap-6">
      {waiting.length === 0 ? null : (
        <section className="flex flex-col gap-2">
          <Heading level={2} size="section">
            {t("rules.list.waiting")}
          </Heading>
          <List label={t("rules.list.waiting")}>{waiting.map(row)}</List>
        </section>
      )}
      {inboxIds.map((inboxId) => {
        const name =
          inboxes.data?.find((inbox) => inbox.id === inboxId)?.address ?? t("rules.list.inbox");
        const list = rules.data.filter(
          (rule) => rule.inbox_id === inboxId && rule.state !== "proposed",
        );
        return (
          <section key={inboxId} className="flex flex-col gap-2">
            <Heading level={2} size="section">
              {name}
            </Heading>
            <List label={name}>{list.map(row)}</List>
          </section>
        );
      })}
    </div>
  );
}

function RuleRow({
  workspaceId,
  rule,
  locale,
  author,
  teaches,
}: {
  workspaceId: string;
  rule: RuleView;
  locale: string;
  author: RuleAuthor | undefined;
  teaches: boolean;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { removeRule, keepRule, declineRule, removing, keeping, declining } =
    useRuleActions(workspaceId);
  const [confirming, setConfirming] = useState(false);
  const [putBack, setPutBack] = useState<"back" | "keep">("back");
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const facts = [
    // Why it is paused first: the line is cut short on a narrow row.
    rule.state === "paused"
      ? t(`rules.list.paused.${rule.paused_reason ?? "author_cannot_write"}`)
      : "",
    author === undefined ? "" : originLine(t, rule, author),
    rule.applied_count > 0
      ? t("rules.list.applied", { count: rule.applied_count, n: number(rule.applied_count) })
      : "",
    rule.last_applied_at === null || rule.last_applied_at === undefined
      ? ""
      : t("rules.list.lastActed", { time: listTime(rule.last_applied_at, locale) }),
    rule.recent_exceptions > 0
      ? t("rules.list.exceptions", {
          count: rule.recent_exceptions,
          n: number(rule.recent_exceptions),
        })
      : "",
  ].filter((fact) => fact !== "");

  const trailing =
    rule.state === "proposed" ? (
      teaches ? (
        <>
          <Button
            variant="primary"
            disabled={keeping}
            onClick={() => {
              keepRule(rule);
            }}
          >
            {t("rules.list.keep")}
          </Button>
          {/* Like saying yes, anyone who may teach here may say no: it does not act yet. */}
          <Button
            variant="ghost"
            disabled={declining}
            onClick={() => {
              declineRule(rule);
            }}
          >
            {t("rules.list.no")}
          </Button>
        </>
      ) : undefined
    ) : rule.may_change && !confirming ? (
      <Button
        variant="ghost"
        onClick={() => {
          setConfirming(true);
        }}
      >
        {t("rules.list.remove")}
      </Button>
    ) : undefined;

  return (
    <ListRow
      title={ruleSentence(t, locale, rule)}
      badge={rule.shared ? t("rules.list.forEveryone") : undefined}
      meta={facts.join(" · ")}
      muted={rule.state === "paused"}
      trailing={trailing}
    >
      {confirming ? (
        <InlineConfirm
          confirmLabel={t("rules.list.confirmRemove")}
          cancelLabel={t("rules.list.cancel")}
          busy={removing}
          onConfirm={() => {
            removeRule(rule, {
              restore: rule.applied_count > 0 && putBack === "back",
              onDone: () => {
                setConfirming(false);
              },
            });
          }}
          onCancel={() => {
            setConfirming(false);
          }}
        >
          {t("rules.list.removeQuestion")}
          {rule.applied_count > 0 ? (
            <ChoiceCards
              label={t("rules.list.changedMail")}
              hideLabel
              value={putBack}
              onChange={setPutBack}
              options={[
                {
                  value: "back",
                  title: t("rules.list.putBack", {
                    count: rule.applied_count,
                    n: number(rule.applied_count),
                  }),
                },
                { value: "keep", title: t("rules.list.leave") },
              ]}
            />
          ) : null}
        </InlineConfirm>
      ) : null}
    </ListRow>
  );
}

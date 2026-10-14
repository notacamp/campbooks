import { useState } from "react";
import { useTranslation } from "react-i18next";

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

import { useInboxes } from "../../api/queries";
import { useRuleMutations, useRules, type RuleView } from "../../api/rules";
import { listTime, type Translate } from "../../model";
import { originLine, ruleSentence } from "../../rules-text";

/**
 * The taught rules of the inboxes the member can see, as sentences: the ones
 * waiting for a yes first (Keep this rule / No), then each inbox's rules with
 * where they came from and how often they acted, and Remove (optionally
 * putting back what they changed). The rules slot of Scout's memory.
 * @public
 */
export function RulesList({ workspaceId }: { workspaceId: string }) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const rules = useRules(workspaceId);
  const inboxes = useInboxes(workspaceId);
  const locale = i18n.language;

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
  const waiting = rules.data.filter((rule) => rule.state === "proposed");
  const byInbox = (inboxes.data ?? []).map((inbox) => ({
    inbox,
    rules: rules.data.filter((rule) => rule.inbox_id === inbox.id && rule.state !== "proposed"),
  }));
  return (
    <div className="flex flex-col gap-6">
      {waiting.length === 0 ? null : (
        <section className="flex flex-col gap-2">
          <Heading level={2} size="section">
            {t("rules.list.waiting")}
          </Heading>
          <List label={t("rules.list.waiting")}>
            {waiting.map((rule) => (
              <RuleRow key={rule.id} workspaceId={workspaceId} rule={rule} locale={locale} />
            ))}
          </List>
        </section>
      )}
      {byInbox
        .filter((group) => group.rules.length > 0)
        .map(({ inbox, rules: list }) => (
          <section key={inbox.id} className="flex flex-col gap-2">
            <Heading level={2} size="section">
              {inbox.address}
            </Heading>
            <List label={inbox.address}>
              {list.map((rule) => (
                <RuleRow key={rule.id} workspaceId={workspaceId} rule={rule} locale={locale} />
              ))}
            </List>
          </section>
        ))}
    </div>
  );
}

function RuleRow({
  workspaceId,
  rule,
  locale,
}: {
  workspaceId: string;
  rule: RuleView;
  locale: string;
}) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const { remove, confirm } = useRuleMutations(workspaceId);
  const [removing, setRemoving] = useState(false);
  const [putBack, setPutBack] = useState<"back" | "keep">("back");
  const number = (n: number) => new Intl.NumberFormat(locale).format(n);
  const facts = [
    originLine(t, rule),
    rule.applied_count > 0 ? t("rules.list.applied", { count: number(rule.applied_count) }) : "",
    rule.last_applied_at === null || rule.last_applied_at === undefined
      ? ""
      : listTime(rule.last_applied_at, locale),
    rule.recent_exceptions > 0
      ? t("rules.list.exceptions", { count: number(rule.recent_exceptions) })
      : "",
    rule.state === "paused" ? t("rules.list.paused") : "",
  ].filter((fact) => fact !== "");

  const trailing =
    rule.state === "proposed" ? (
      <>
        <Button
          variant="primary"
          disabled={confirm.isPending}
          onClick={() => {
            confirm.mutate({ ruleId: rule.id });
          }}
        >
          {t("rules.list.keep")}
        </Button>
        <Button
          variant="ghost"
          onClick={() => {
            remove.mutate({ ruleId: rule.id, restore: false });
          }}
        >
          {t("rules.list.no")}
        </Button>
      </>
    ) : rule.may_change && !removing ? (
      <Button
        variant="ghost"
        onClick={() => {
          setRemoving(true);
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
      {removing ? (
        <InlineConfirm
          confirmLabel={t("rules.list.confirmRemove")}
          cancelLabel={t("rules.list.cancel")}
          busy={remove.isPending}
          onConfirm={() => {
            remove.mutate({ ruleId: rule.id, restore: putBack === "back" });
            setRemoving(false);
          }}
          onCancel={() => {
            setRemoving(false);
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
                  title: t("rules.list.putBack", { count: number(rule.applied_count) }),
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

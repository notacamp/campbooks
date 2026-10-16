import { useTranslation } from "react-i18next";

import { RulePreview, type RulePreviewSample } from "~/lib/ui";

import { usePreviews, type RuleShape } from "../../api/queries";
import { ruleSentence, type Names, type Translate } from "../../model";

export interface RuleMatchProps {
  workspaceId: string;
  rule: RuleShape;
  /** The inboxes it would be taught in, to count matches in. */
  inboxIds: readonly string[];
  /** Display names of senders, by address. */
  names?: Names | undefined;
  /** Label names, by id. */
  labels?: Names | undefined;
}

/**
 * A rule before it is saved: its sentence, and what it matches in the given
 * inboxes, counted on the server (`conversations.preview_rule`), with a few
 * of those emails.
 */
export function RuleMatch({ workspaceId, rule, inboxIds, names, labels }: RuleMatchProps) {
  const { t: typed, i18n } = useTranslation("assistant");
  const t = typed as unknown as Translate;
  const previews = usePreviews(workspaceId, inboxIds, rule);
  const pending = inboxIds.length > 0 && previews.some((preview) => preview.isPending);
  const settled = previews.flatMap((preview) => (preview.data === undefined ? [] : [preview.data]));
  const matching = settled.reduce((sum, preview) => sum + preview.matching, 0);
  const day = new Intl.DateTimeFormat(i18n.language, { day: "numeric", month: "short" });
  const samples: RulePreviewSample[] = settled
    .flatMap((preview) => preview.samples)
    .toSorted((a, b) => b.sent_at.localeCompare(a.sent_at))
    .slice(0, 3)
    .map((sample) => ({
      subject: sample.subject,
      sender: names?.get(sample.sender) ?? sample.sender,
      when: day.format(new Date(sample.sent_at)),
    }));
  const summary =
    settled.length === 0
      ? undefined
      : matching === 0
        ? t("rule.noMatches")
        : t("rule.matches", { count: matching });
  return (
    <RulePreview
      tag={t("rule.tag")}
      sentence={ruleSentence(rule, t, i18n.language, names, labels)}
      summary={summary}
      samples={samples}
      samplesLabel={t("rule.samples")}
      pending={pending}
    />
  );
}

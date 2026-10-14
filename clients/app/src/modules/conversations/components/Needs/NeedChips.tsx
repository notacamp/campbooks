import { useTranslation } from "react-i18next";

import { Pill, Tag } from "~/lib/ui";

import type { NeedView } from "../../api/queries";
import type { Translate } from "../../model";
import { dueLabel, needChip, needSignal } from "../../needs";

/** What a conversation needs, as chips: the reason, and its due date when it has one. */
export function NeedChips({ need, now }: { need: NeedView; now?: Date | undefined }) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const due = dueLabel(t, need, i18n.language, now);
  return (
    <>
      <Pill signal={needSignal(need)}>{needChip(t, need)}</Pill>
      {due === undefined ? null : <Tag tone={due.overdue ? "danger" : "neutral"}>{due.text}</Tag>}
    </>
  );
}

import { useTranslation } from "react-i18next";

import { useRule, useRuleNotice, type RuleNotice } from "./api/rules";
import type { Translate } from "./model";
import { ruleSentence } from "./rules-text";

/** What a chat with Scout is about, in one line, or why it can't be said. */
interface ChatSubject {
  /** The line, once read. */
  text: string | undefined;
  /** It could not be read: gone, or no longer yours to see. */
  failed: boolean;
}

/**
 * The sender of one of Scout's suggestions: their name, then the address
 * (which the sender does not choose), or the address alone.
 */
export function noticeSender(notice: Pick<RuleNotice, "sender" | "sender_name">): string {
  const name = notice.sender_name?.trim();
  return name === undefined || name === "" || name === notice.sender
    ? notice.sender
    : `${name} (${notice.sender})`;
}

/**
 * A rule as a chat about it names it: its sentence, as the rules list
 * writes it (campbooks-cloud#305). Asks for nothing without a rule.
 */
export function useRuleSubject(workspaceId: string, ruleId: string | undefined): ChatSubject {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const rule = useRule(workspaceId, ruleId);
  return {
    text: rule.data === undefined ? undefined : ruleSentence(t, i18n.language, rule.data),
    failed: rule.isError,
  };
}

/**
 * One of Scout's suggestions as a chat about it names it: its sender. Asks
 * for nothing without a suggestion.
 */
export function useRuleNoticeSubject(
  workspaceId: string,
  noticeId: string | undefined,
): ChatSubject {
  const notice = useRuleNotice(workspaceId, noticeId);
  return {
    text: notice.data === undefined ? undefined : noticeSender(notice.data),
    failed: notice.isError,
  };
}

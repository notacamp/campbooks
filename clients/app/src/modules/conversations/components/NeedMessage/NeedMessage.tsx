import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Clamp } from "~/lib/ui";

import { useConversation, type ConversationSummary, type NeedView } from "../../api/queries";
import { avatarKind, type Translate } from "../../model";
import { ThreadMessage } from "../Reader/ThreadMessage";

export interface NeedMessageProps {
  workspaceId: string;
  conversation: ConversationSummary;
  need: NeedView;
  now?: Date | undefined;
}

/**
 * The email behind a need, as the reader shows it (sanitised, images held
 * back, links leading nowhere when it may be phishing), a few lines deep
 * with "Show the whole email" to read it in place. Home's opened item.
 */
export function NeedMessage({ workspaceId, conversation, need, now }: NeedMessageProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const detail = useConversation(workspaceId, conversation.id);
  const [showImages, setShowImages] = useState(false);
  const messages = detail.data?.messages ?? [];
  // The message the need is about, or the newest one the sender wrote.
  const message =
    messages.find((m) => m.id === need.message_id) ??
    messages.findLast((m) => m.direction !== "outbound");
  if (message === undefined) {
    return null;
  }
  const suspicious = need.question === "suspicious";
  return (
    <Clamp moreLabel={t("needs.mail.more")} lessLabel={t("needs.mail.less")}>
      <ThreadMessage
        workspaceId={workspaceId}
        conversationId={conversation.id}
        message={message}
        open
        onOpen={() => undefined}
        showImages={showImages}
        onShowImages={() => {
          setShowImages(true);
        }}
        kind={avatarKind(conversation)}
        inertLinks={suspicious}
        note={suspicious ? t("needs.mail.linksOff") : undefined}
        now={now}
      />
    </Clamp>
  );
}

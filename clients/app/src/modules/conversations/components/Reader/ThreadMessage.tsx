import { useTranslation } from "react-i18next";

import { Button, FoldedMessage, MessageCard, SafeHtml, Skeleton, Text } from "~/lib/ui";

import { useMessageBody, type MessageView } from "../../api/queries";
import { displayName, messageTime, recipientsLine, type Translate } from "../../model";

export interface ThreadMessageProps {
  workspaceId: string;
  conversationId: string;
  message: MessageView;
  /** Folded to one line until opened. */
  open: boolean;
  onOpen: () => void;
  /** The member asked to see this message's remote images. */
  showImages: boolean;
  onShowImages: () => void;
  kind: "person" | "service";
  now?: Date | undefined;
}

/**
 * One message of the thread: its plain text, or its HTML body (sanitised by
 * the server) in the sandboxed frame, with "Show images" when remote images
 * were held back, and its attachments.
 */
export function ThreadMessage({
  workspaceId,
  conversationId,
  message,
  open,
  onOpen,
  showImages,
  onShowImages,
  kind,
  now,
}: ThreadMessageProps) {
  const { t: typed, i18n } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const body = useMessageBody(
    workspaceId,
    conversationId,
    message.id,
    showImages,
    open && message.has_html,
  );
  const from = displayName(message.from);
  const time = messageTime(message.sent_at, i18n.language, now);
  const text = message.text?.content ?? "";

  if (!open) {
    return (
      <FoldedMessage
        from={from}
        avatar={{ kind }}
        time={time}
        dateTime={message.sent_at}
        snippet={text.replaceAll(/\s+/gu, " ").trim()}
        onOpen={onOpen}
      />
    );
  }

  const files = message.attachments
    .filter((attachment) => !attachment.inline)
    .map((attachment) => attachment.filename);
  const remote = body.data?.remote_images ?? 0;
  const html = body.data?.html;

  let content;
  if (!message.has_html) {
    content = text;
  } else if (body.isPending) {
    content = (
      <span className="flex flex-col gap-2" aria-busy="true">
        <Skeleton width="90%" />
        <Skeleton width="70%" />
        <Skeleton width="80%" />
      </span>
    );
  } else if (html === undefined || html === null) {
    content = body.isError ? <Text tone="tertiary">{t("error.body")}</Text> : text;
  } else {
    content = (
      <SafeHtml
        html={html}
        title={t("reader.frameTitle", { name: from })}
        remoteImages={body.data?.images_shown ?? false}
      />
    );
  }

  const truncated = message.text?.truncated === true || body.data?.truncated === true;
  return (
    <MessageCard
      from={from}
      avatar={{ kind }}
      time={time}
      dateTime={message.sent_at}
      recipients={recipientsLine(t, message, i18n.language)}
      rich={message.has_html}
      attachments={files}
      actions={
        remote > 0 || truncated ? (
          <>
            {truncated ? (
              <Text tone="tertiary" size="sm">
                {t("reader.truncated")}
              </Text>
            ) : null}
            {remote > 0 && body.data?.images_shown !== true ? (
              <>
                <Text tone="tertiary" size="sm">
                  {t("actions.imagesHidden", { count: remote })}
                </Text>
                <Button
                  variant="secondary"
                  icon="image"
                  shortcut="inbox.show_images"
                  onClick={onShowImages}
                >
                  {t("actions.showImages")}
                </Button>
              </>
            ) : null}
          </>
        ) : undefined
      }
    >
      {content}
    </MessageCard>
  );
}

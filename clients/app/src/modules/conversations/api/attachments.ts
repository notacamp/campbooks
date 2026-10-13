import { api, browserUrl, unwrap } from "~/lib/api";

import { cleanFileName, previewType } from "../model";

/** Which attachment: the path of `conversations.get_attachment`. */
export interface AttachmentRef {
  workspaceId: string;
  conversationId: string;
  messageId: string;
  attachmentId: string;
}

const PATH =
  "/api/v1/w/{workspace_id}/conversations/{conversation_id}/messages/{message_id}/attachments/{attachment_id}" as const;

function pathOf(ref: AttachmentRef): string {
  return `/api/v1/w/${ref.workspaceId}/conversations/${ref.conversationId}/messages/${ref.messageId}/attachments/${ref.attachmentId}`;
}

/** `conversations.get_attachment`: the bytes, through the API client. */
async function fetchBytes(ref: AttachmentRef): Promise<Blob> {
  return unwrap(
    await api.GET(PATH, {
      params: {
        path: {
          workspace_id: ref.workspaceId,
          conversation_id: ref.conversationId,
          message_id: ref.messageId,
          attachment_id: ref.attachmentId,
        },
      },
      parseAs: "blob",
    }),
  );
}

/**
 * Save the attachment: its bytes as an opaque download (never a type the
 * browser could open), under its cleaned name.
 */
export async function downloadAttachment(ref: AttachmentRef, name: string): Promise<void> {
  const bytes = await fetchBytes(ref);
  const url = URL.createObjectURL(new Blob([bytes], { type: "application/octet-stream" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = cleanFileName(name);
  link.rel = "noopener";
  document.body.append(link);
  link.click();
  link.remove();
  // Give the browser the time to start the download before letting go.
  setTimeout(() => {
    URL.revokeObjectURL(url);
  }, 30_000);
}

/**
 * A picture to preview: a `blob:` URL whose type is set here from the
 * client's raster list, so it can only ever be shown as an image (a type
 * off the list is refused). Revoke it when done.
 */
export async function imageUrl(ref: AttachmentRef, contentType: string): Promise<string> {
  const type = previewType(contentType);
  if (type === undefined) {
    throw new TypeError("not a raster image");
  }
  const bytes = await fetchBytes(ref);
  return URL.createObjectURL(new Blob([bytes], { type }));
}

/**
 * Open a PDF in the browser's own viewer, in a new tab: served inline under
 * a `sandbox` policy, so it runs at an opaque origin. Returns `false` where
 * the browser cannot open API addresses itself (the native client).
 */
export function openPdf(ref: AttachmentRef): boolean {
  const url = browserUrl(`${pathOf(ref)}?inline=true`);
  if (url === undefined) {
    return false;
  }
  const link = document.createElement("a");
  link.href = url;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  document.body.append(link);
  link.click();
  link.remove();
  return true;
}

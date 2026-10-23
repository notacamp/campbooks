import { useNavigate } from "@tanstack/react-router";
import type { MouseEvent } from "react";

/** The reader, on the message a calendar entry was read from. */
export const emailPath = (workspaceId: string, stream: string, conversationId: string) =>
  `/w/${workspaceId}/inbox/${stream}/${conversationId}`;

/** An in-app link: a real href, followed without a reload. */
export function useInAppLink() {
  const navigate = useNavigate();
  return (href: string) => ({
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (event.metaKey || event.ctrlKey || event.shiftKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      void navigate({ to: href });
    },
  });
}

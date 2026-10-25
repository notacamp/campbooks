// Public surface of the `schedule` module (mirrors crates/modules/schedule).
// Other modules and the app shell import from here only.
import { createElement, type ReactNode } from "react";

import { FoundInMessage } from "./components/Found/FoundInMessage";

export { useFreeSlots, useWritableCalendars } from "./api/queries";
export { buildScheduleRoutes } from "./routes";
export { SCHEDULE_SHORTCUTS } from "./shortcuts";

/**
 * "Scout noticed" under a message's header in the reader (the conversations
 * module hands it each received message): what was read from it, as events
 * on the member's calendar.
 */
export function renderFoundInMessage(found: {
  workspaceId: string;
  conversationId: string;
  messageId: string;
  subject: string;
}): ReactNode {
  return createElement(FoundInMessage, { ...found, key: found.messageId });
}

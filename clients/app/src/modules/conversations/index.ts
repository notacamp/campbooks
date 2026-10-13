// Public surface of the `conversations` module (mirrors crates/modules/conversations).
// Other modules and the app shell import from here only.
export { buildConversationsRoutes } from "./routes";
export { CONVERSATIONS_SHORTCUTS } from "./shortcuts";
export { COMPOSE_SHORTCUTS } from "./compose/shortcuts";
export { StreamNav, streamPath } from "./components/StreamNav/StreamNav";
export {
  useConversation,
  useConversationsByIds,
  type ConversationSummary,
  type NeedView,
} from "./api/queries";
export { useNeedActions, type NeedActions } from "./components/Needs/use-need-actions";
export { NeedChips } from "./components/Needs/NeedChips";
export {
  dueLabel,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  warningReasons,
} from "./needs";
export { useInboxes } from "./api/queries";

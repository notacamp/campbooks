// Public surface of the `conversations` module (mirrors crates/modules/conversations).
// Other modules and the app shell import from here only.
export { buildConversationsRoutes } from "./routes";
export { CONVERSATIONS_SHORTCUTS } from "./shortcuts";
export { COMPOSE_SHORTCUTS } from "./compose/shortcuts";
export { StreamNav, streamPath } from "./components/StreamNav/StreamNav";
export {
  useConversation,
  useConversationsByIds,
  useConversationSummaries,
  type ConversationSummary,
  type NeedView,
} from "./api/queries";
export { useNeedActions } from "./components/Needs/use-need-actions";
export {
  dueLabel,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  warningReasons,
} from "./needs";
export { useInboxes } from "./api/queries";
// What the search results page reuses: the reader, the inbox actions and the
// reply composer, so a hit opens and is tidied exactly as in the inbox.
export { ConversationReader } from "./components/Reader/ConversationReader";
export { useInboxActions } from "./components/InboxScreen/use-inbox-actions";
export { renderReplyComposer } from "./components/ReplyComposer/ReplyComposer";
export { ReplyDialog, type ReplyResume } from "./components/ReplyDialog/ReplyDialog";
export { displayName, listTime } from "./model";
export { isStream, type Stream } from "./api/queries";
/**
 * The rules slot of Scout's memory: the assistant module's page fills it
 * (campbooks-cloud#121), so it has no caller here yet.
 * @public
 */
export { RulesList } from "./components/Rules/RulesList";

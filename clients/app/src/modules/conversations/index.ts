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
// Smart rules (campbooks-cloud#169): the confirmation every rule goes
// through, the offers after Report and delete and Already paid, and "Scout
// noticed" at the bottom of Home.
export { useAheadOffer } from "./components/SmartRule/AheadOffer";
export { ScoutNoticed } from "./components/SmartRule/ScoutNoticed";
export { NeedMessage, needMessageOf } from "./components/NeedMessage/NeedMessage";
export {
  awaitedOf,
  billKind,
  howRows,
  needSender,
  needSentence,
  quoteAttribution,
  unsureNote,
  unsureOf,
  warningReasons,
} from "./needs";
export { useInboxes } from "./api/queries";
// Where tasks made from conversations came from (the Tasks place, via the app).
export { useConversationSources } from "./api/task-sources";
// What the search results page reuses: the reader, the inbox actions and the
// reply composer, so a hit opens and is tidied exactly as in the inbox.
export { ConversationReader } from "./components/Reader/ConversationReader";
export { useInboxActions } from "./components/InboxScreen/use-inbox-actions";
export { renderReplyComposer } from "./components/ReplyComposer/ReplyComposer";
export { ReplyDialog, type ReplyResume } from "./components/ReplyDialog/ReplyDialog";
// Nudge from Home (campbooks-cloud#293): Scout's few lines to the person a
// conversation waits on.
export { NudgeDialog, type NudgeResume } from "./components/NudgeDialog/NudgeDialog";
// Forward from Home (campbooks-cloud#166): find the person by name, then
// finish the forward in the composer.
export { ForwardDialog } from "./components/ForwardDialog/ForwardDialog";
export { displayName, listTime, type Move } from "./model";
// What Scout's chat names when it is about a rule or one of Scout's
// suggestions (campbooks-cloud#305).
export { useRuleNoticeSubject, useRuleSubject } from "./scout-subjects";
export { isPlace, isStream, type Place, type Stream } from "./api/queries";
/**
 * The rules slot of Scout's memory: the assistant module's page fills it
 * (campbooks-cloud#121), so it has no caller here yet.
 * @public
 */
export { RulesList } from "./components/Rules/RulesList";
/**
 * The people slot of Scout's memory: what the member told Scout about
 * people (campbooks-cloud#272), filled by the assistant module's page.
 * @public
 */
export { PersonNotesList } from "./components/PersonNotes/PersonNotesList";

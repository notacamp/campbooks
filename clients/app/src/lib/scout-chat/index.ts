// Opening Scout's chat from any screen (campbooks-cloud#305): the app
// provides the opener, the assistant module draws the panel, and other
// modules ask for it here without importing that module.
export {
  conversationRef,
  pageRef,
  ruleNoticeRef,
  ruleRef,
  SCOUT_CHAT_ABOUT,
  taskRef,
  ScoutChatHost,
  ScoutChatProvider,
  useScoutChat,
  type OpenScoutChat,
} from "./scout-chat";

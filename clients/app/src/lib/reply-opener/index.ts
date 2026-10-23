// Opening a conversation's Reply dialog from any screen (campbooks-cloud#305):
// the app provides the opener and the conversations module draws the
// dialog; Scout's chat asks for it here without importing that module.
export {
  ReplyOpenerProvider,
  useOpenReply,
  type OpenReply,
  type ReplyToOpen,
} from "./reply-opener";

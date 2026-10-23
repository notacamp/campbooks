// Public surface of the `assistant` module (mirrors crates/modules/assistant).
// Other modules and the app shell import from here only.
export { buildAssistantRoutes } from "./routes";
export { ASSISTANT_SHORTCUTS } from "./shortcuts";
export { ScoutOffer } from "./components/ScoutOffer/ScoutOffer";
// Scout's chat (campbooks-cloud#305): the app mounts it once per workspace
// and opens it through `~/lib/scout-chat`.
export { ScoutChat } from "./components/ScoutChat/ScoutChat";

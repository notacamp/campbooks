// Public surface of the design system for Campbooks. Modules and the app
// shell import UI only from here (dependency-cruiser: "ui-public-surface"):
// everything in @notacamp/ui (clients/ui, the shared design system), plus
// the components that name a Campbooks concept, which stay in the app and
// are built from the package's. Styling reads only the tokens (stylelint).
import "@notacamp/ui/fonts.css";
import "@notacamp/ui/tokens.css";
import "@notacamp/ui/base.css";

export * from "@notacamp/ui";
// The app's own wiring of the package: its preferences, its strings.
export { APP_THEME_PREFS, ThemeProvider } from "./theme";
export { UI_NAMESPACES, type UiMessages } from "./messages";

export {
  AttachmentList,
  focusNextAttachment,
  ImagePreview,
  type AttachmentKind,
  type AttachmentListItem,
  type AttachmentListProps,
  type ImagePreviewProps,
} from "./components/Attachments/Attachments";
// Tables (cloud#365): the grid, the board, the row peek and their small parts.
export {
  OptionTag,
  TypeGlyph,
  CellButton,
  CellEmpty,
  ColumnMark,
  HeaderButton,
  SummaryButton,
  CellInput,
  CellLine,
  CellRow,
  CellLink,
  type ColumnMarkProps,
  type HeaderButtonProps,
  type SummaryButtonProps,
  type OptionTagColor,
  type OptionTagProps,
  type TypeGlyphProps,
} from "./components/Table/Cells";
export {
  DataGrid,
  type CellPos,
  type CellState,
  type DataGridProps,
  type GridColumn,
  type GridRow,
} from "./components/Table/DataGrid";
export { Board, type BoardCard, type BoardGroup, type BoardProps } from "./components/Table/Board";
export {
  CheckGroup,
  MenuHeading,
  MenuItem,
  MenuList,
  MenuSeparator,
  type CheckGroupProps,
  type MenuItemProps,
  type MenuListProps,
} from "./components/Table/Menu";
export { Picker, type PickerItem, type PickerProps } from "./components/Table/Picker";
export {
  PropertyLine,
  PropertyList,
  RowPeek,
  type PropertyLineProps,
  type RowPeekProps,
} from "./components/Table/Peek";
export {
  FilterChip,
  TableToolbar,
  ToolbarButton,
  type FilterChipProps,
  type TableToolbarProps,
  type ToolbarButtonProps,
  type TableTab,
} from "./components/Table/Toolbar";
// The Pages editor's own pieces (cloud#363).
export {
  PageTree,
  PageTreeGroup,
  PageTreeRow,
  type PageTreeGroupProps,
  type PageTreeProps,
  type PageTreeRowProps,
} from "./components/PageTree/PageTree";
export {
  DocCard,
  DocImage,
  DocToggle,
  MentionChip,
  RichDocument,
  type DocCardProps,
  type DocImageProps,
  type DocToggleProps,
  type MentionChipProps,
  type MentionKind,
  type RichDocumentProps,
} from "./components/RichDocument/RichDocument";
export {
  ChatAbout,
  ChatField,
  ChatMessage,
  ChatNotice,
  ChatPanel,
  ChatProposal,
  ChatRecent,
  ChatSteps,
  ChatSuggestions,
  ChatText,
  ChatWorking,
  type ChatAboutProps,
  type ChatFieldProps,
  type ChatMessageProps,
  type ChatNoticeProps,
  type ChatPanelProps,
  type ChatProposalProps,
  type ChatProposalState,
  type ChatRecentItem,
  type ChatRecentProps,
  type ChatStep,
  type ChatStepRecord,
  type ChatStepsProps,
  type ChatSuggestionsProps,
  type ChatWorkingProps,
} from "./components/Chat/Chat";
export { Composer, type ComposerProps } from "./components/Composer/Composer";
export {
  ConversationRow,
  type ConversationRowProps,
  type RowLeaving,
} from "./components/ConversationRow/ConversationRow";
export {
  HowDecided,
  type HowDecidedProps,
  type HowDecidedRow,
} from "./components/HowDecided/HowDecided";
export {
  Attachment,
  FoldedMessage,
  MessageCard,
  MetaLine,
  MoreMessages,
  QuotedText,
  type FoldedMessageProps,
  type MessageCardProps,
  type MessageSide,
  type MoreMessagesProps,
  type QuotedTextProps,
} from "./components/Message/Message";
export {
  MessageComposer,
  type ComposerAttachment,
  type MessageComposerProps,
  type RecipientFieldName,
  type RecipientSuggester,
} from "./components/MessageComposer/MessageComposer";
export { sendTimeLabel } from "./components/MessageComposer/send-time";
export { Offer, type OfferProps } from "./components/Offer/Offer";
export { ProviderMark, type ProviderMarkProps } from "./components/ProviderMark/ProviderMark";
export {
  AsideSection,
  Figure,
  FigureRow,
  HomeLayout,
  Line,
  LineMeta,
  PageHero,
  QueueGroup,
  type LineProps,
  type PageHeroProps,
  type QueueGroupProps,
} from "./components/Queue/Queue";
export {
  isEmailAddress,
  parseRecipients,
  RecipientField,
  type Recipient,
  type RecipientFieldProps,
  type RecipientSuggestion,
} from "./components/RecipientField/RecipientField";
export {
  AskLine,
  ReplyDraft,
  ReplyQuote,
  type AskLineProps,
  type ReplyDraftProps,
  type ReplyQuoteProps,
} from "./components/ReplyDraft/ReplyDraft";
export {
  RulePreview,
  type RulePreviewProps,
  type RulePreviewSample,
} from "./components/RulePreview/RulePreview";
export { TaskRow, type TaskRowChip, type TaskRowProps } from "./components/TaskRow/TaskRow";
export { ThinkingTrace, type ThinkingTraceProps } from "./components/ThinkingTrace/ThinkingTrace";
export {
  WorkspaceMenu,
  type WorkspaceMenuItem,
  type WorkspaceMenuProps,
} from "./components/WorkspaceMenu/WorkspaceMenu";

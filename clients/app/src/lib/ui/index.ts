// Public surface of the design system. Modules and the app shell import UI
// only from here (dependency-cruiser: "ui-public-surface"); styling lives in
// the components and reads only the tokens in ./tokens (stylelint).
import "@fontsource-variable/funnel-sans";
import "@fontsource-variable/funnel-display";
import "./tokens/tokens.css";
import "./styles/base.css";

export {
  ACCENTS,
  cssVar,
  DENSITIES,
  SIGNALS,
  THEME_MODES,
  type Accent,
  type Density,
  type Signal,
  type ThemeMode,
  type ComponentToken,
  type SemanticToken,
  type Token,
} from "./tokens/tokens";
export { applyTheme, DEFAULT_THEME, type ThemeSettings } from "./theme/theme";
export {
  ThemeProvider,
  useTheme,
  type ThemeContextValue,
  type ThemeProviderProps,
} from "./theme/ThemeProvider";
export { UI_NAMESPACES, type UiMessages } from "./messages";

export { ActionBar, type ActionBarProps } from "./components/ActionBar/ActionBar";
export {
  AttachmentList,
  focusNextAttachment,
  ImagePreview,
  type AttachmentKind,
  type AttachmentListItem,
  type AttachmentListProps,
  type ImagePreviewProps,
} from "./components/Attachments/Attachments";
export { Avatar, avatarHue, initialsOf, type AvatarProps } from "./components/Avatar/Avatar";
export { Banner, type BannerProps } from "./components/Banner/Banner";
export {
  Button,
  ButtonLink,
  type ButtonProps,
  type ButtonLinkProps,
  type ButtonSize,
  type ButtonVariant,
} from "./components/Button/Button";
export { Checkbox, type CheckboxProps } from "./components/Checkbox/Checkbox";
export {
  CheckCards,
  type CheckCardOption,
  type CheckCardsProps,
} from "./components/CheckCards/CheckCards";
export {
  Highlight,
  type HighlightProps,
  type HighlightSegment,
} from "./components/Highlight/Highlight";
export {
  Chip,
  ChipDivider,
  ChipLink,
  ChipRow,
  type ChipLinkProps,
  type ChipProps,
} from "./components/Chip/Chip";
export {
  ChoiceCards,
  type ChoiceCardOption,
  type ChoiceCardsProps,
} from "./components/ChoiceCards/ChoiceCards";
export {
  CommandMenu,
  type Command,
  type CommandMenuProps,
  type MenuOption,
  type MenuQuery,
  type MenuSection,
} from "./components/CommandMenu/CommandMenu";
export { CodeBlock, type CodeBlockProps } from "./components/CodeBlock/CodeBlock";
export { Composer, type ComposerProps } from "./components/Composer/Composer";
export { CopyField, type CopyFieldProps } from "./components/CopyField/CopyField";
export {
  ConversationRow,
  type ConversationRowProps,
  type RowLeaving,
} from "./components/ConversationRow/ConversationRow";
export { Disclosure, type DisclosureProps } from "./components/Disclosure/Disclosure";
export {
  HowDecided,
  type HowDecidedProps,
  type HowDecidedRow,
} from "./components/HowDecided/HowDecided";
export { Dot, type DotProps } from "./components/Dot/Dot";
export {
  PrivacyLine,
  SettingsEmpty,
  SettingsPage,
  SettingsSection,
  type SettingsEmptyProps,
  type SettingsPageProps,
  type SettingsSectionProps,
} from "./components/Settings/Settings";
export { Fact, FactList, type FactProps, type FactStatus } from "./components/Facts/Facts";
export { EmptyState, type EmptyStateProps } from "./components/EmptyState/EmptyState";
export {
  Select,
  TextField,
  type SelectOption,
  type SelectProps,
  type TextFieldProps,
} from "./components/Field/Field";
export { Brand, FocusPage, type FocusPageProps } from "./components/FocusPage/FocusPage";
export {
  Dialog,
  DialogChoices,
  type DialogChoice,
  type DialogChoicesProps,
  type DialogProps,
  type DialogResult,
} from "./components/Dialog/Dialog";
export { Icon, ICON_NAMES, type IconName, type IconProps } from "./components/Icon/Icon";
export { IconButton, type IconButtonProps } from "./components/IconButton/IconButton";
export {
  Item,
  ItemDetail,
  SwipeAction,
  type ItemDetailProps,
  type ItemProps,
} from "./components/Item/Item";
export {
  ItemActions,
  type ItemAction,
  type ItemActionsProps,
} from "./components/ItemActions/ItemActions";
export { InlineConfirm, type InlineConfirmProps } from "./components/InlineConfirm/InlineConfirm";
export {
  SearchField,
  type SearchFieldChip,
  type SearchFieldProps,
} from "./components/SearchField/SearchField";
export { Overlay, type OverlayProps } from "./internal/Overlay";
export { Kbd, KbdHint, type KbdHintProps } from "./components/Kbd/Kbd";
export { List, ListRow, type ListRowProps } from "./components/List/List";
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
} from "./components/MessageComposer/MessageComposer";
export { NavDivider, NavItem, type NavItemProps } from "./components/NavItem/NavItem";
export { Offer, type OfferProps } from "./components/Offer/Offer";
export {
  Note,
  type NoteChoice,
  type NoteChoiceOption,
  type NoteLink,
  type NoteProps,
  type NoteQuote,
} from "./components/Note/Note";
export { Pill, type PillProps } from "./components/Pill/Pill";
export { Progress, type ProgressProps } from "./components/Progress/Progress";
export { ProviderMark, type ProviderMarkProps } from "./components/ProviderMark/ProviderMark";
export {
  Popover,
  PopoverSection,
  type PopoverProps,
  type PopoverTriggerProps,
} from "./components/Popover/Popover";
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
} from "./components/RecipientField/RecipientField";
export {
  Segmented,
  type SegmentedOption,
  type SegmentedProps,
} from "./components/Segmented/Segmented";
export {
  AccountButton,
  ListPane,
  PaneHeader,
  PhoneHeader,
  PhoneSubnav,
  ReaderBody,
  ReaderFooter,
  ReaderPane,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  StatusLine,
  WorkspaceMark,
  WorkspaceSwitcher,
  type AccountButtonProps,
  type ReaderFooterProps,
  type WorkspaceSwitcherProps,
} from "./components/Shell/Shell";
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
export { SafeHtml, type SafeHtmlProps } from "./safe-html/SafeHtml";
export { ShortcutSheet, type ShortcutSheetProps } from "./components/ShortcutSheet/ShortcutSheet";
export { Clamp, type ClampProps } from "./components/Clamp/Clamp";
export { Skeleton, SkeletonList, type SkeletonProps } from "./components/Skeleton/Skeleton";
export {
  StatusText,
  type StatusTextProps,
  type StatusTone,
} from "./components/StatusText/StatusText";
export { Steps, type StepsProps } from "./components/Steps/Steps";
export { TabBar, type TabBarItem, type TabBarProps } from "./components/TabBar/TabBar";
export { Tag, type TagProps } from "./components/Tag/Tag";
export {
  Toast,
  ToastRegion,
  UndoProvider,
  useUndoToast,
  type PendingAction,
  type PendingFailure,
  type PendingHandle,
  type UndoableAction,
  type UndoApi,
  type UndoEntry,
} from "./components/Toast/Toast";
export {
  Heading,
  Text,
  VisuallyHidden,
  type HeadingProps,
  type HeadingSize,
  type TextProps,
  type TextTone,
} from "./components/Typography/Typography";
export { Visor, VISOR_STATES, type VisorProps, type VisorState } from "./components/Visor/Visor";
export {
  WorkspaceMenu,
  type WorkspaceMenuItem,
  type WorkspaceMenuProps,
} from "./components/WorkspaceMenu/WorkspaceMenu";

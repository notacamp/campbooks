// Public surface of @notacamp/ui, the Not A Camp design system: tokens,
// themes, components and their messages (the shortcut engine is
// `@notacamp/ui/shortcuts`). Styling lives in the components and reads only
// the tokens (stylelint). The app imports the CSS once: `fonts.css`,
// `tokens.css` (or its own token set on top), `base.css` and, from the
// published build, `ui.css` (README.md).
export {
  ACCENTS,
  cssVar,
  DENSITIES,
  CALENDAR_KINDS,
  SIGNALS,
  THEME_MODES,
  type Accent,
  type Density,
  type CalendarKind,
  type Signal,
  type ThemeMode,
  type ComponentToken,
  type SemanticToken,
  type Token,
} from "./tokens/tokens";
export { applyTheme, DEFAULT_THEME, DEFAULT_WORKSPACE, type ThemeSettings } from "./theme/theme";
export { localStoragePrefs, type ThemePrefKey, type ThemePrefs } from "./theme/prefs";
export {
  ThemeProvider,
  useTheme,
  type ThemeContextValue,
  type ThemeProviderProps,
} from "./theme/ThemeProvider";
export { UI_NAMESPACES, type UiMessages } from "./messages";
export { UI_LOCALES, type Locale, type Messages } from "./locales";

// For components built on the design system (in an app, next to these).
export { cx } from "./internal/cx";
export { formatSize } from "./internal/format-size";
export { useFocusTrap } from "./internal/use-focus-trap";
export { ActionBar, type ActionBarProps } from "./components/ActionBar/ActionBar";
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
export {
  Agenda,
  AgendaDay,
  AgendaRow,
  CalendarColour,
  CalendarDot,
  CalendarHeader,
  DayStrip,
  EventSheet,
  FoundItem,
  FoundReading,
  FoundStrip,
  KIND_ICONS,
  KindChip,
  KindChipButton,
  KindDot,
  KindIcon,
  MonthGrid,
  SheetActions,
  SheetFact,
  SheetFacts,
  SheetNotice,
  SheetQuote,
  SheetSection,
  SheetTitle,
  SheetTop,
  SourceLink,
  ViewTabs,
  WeekGrid,
  type AgendaDayProps,
  type AgendaRowProps,
  type CalendarColourProps,
  type CalendarHeaderProps,
  type DayStripDay,
  type FoundItemProps,
  type FoundStripProps,
  type KindDotProps,
  type MonthDay,
  type MonthEvent,
  type MonthGridProps,
  type SourceLinkProps,
  type ViewTabsProps,
  type WeekBlock,
  type WeekDayHead,
  type WeekGridProps,
  type WeekPill,
} from "./components/Calendar/Calendar";
// A document editor's pieces (Campbooks' Pages).
export { BlockHandle, type BlockHandleProps } from "./components/BlockHandle/BlockHandle";
export {
  FormatBar,
  type FormatBarItem,
  type FormatBarProps,
} from "./components/FormatBar/FormatBar";
export {
  SuggestionMenu,
  type SuggestionGroup,
  type SuggestionItem,
  type SuggestionMenuProps,
} from "./components/SuggestionMenu/SuggestionMenu";
export { TitleField, type TitleFieldProps } from "./components/TitleField/TitleField";
export { Switch, type SwitchProps } from "./components/Switch/Switch";
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
export { CopyField, type CopyFieldProps } from "./components/CopyField/CopyField";
export { DatePicker, type DatePickerProps } from "./components/DatePicker/DatePicker";
export { Disclosure, type DisclosureProps } from "./components/Disclosure/Disclosure";
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
  OverflowActions,
  type OverflowAction,
  type OverflowActionsProps,
} from "./components/OverflowActions/OverflowActions";
export {
  Item,
  ItemDetail,
  SwipeAction,
  type ItemDetailProps,
  type ItemEyebrow,
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
export { NavDivider, NavItem, type NavItemProps } from "./components/NavItem/NavItem";
export { NoteField, type NoteFieldProps } from "./components/NoteField/NoteField";
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
export {
  Popover,
  PopoverSection,
  type PopoverProps,
  type PopoverTriggerProps,
} from "./components/Popover/Popover";
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
export { TimePicker, type TimePickerProps } from "./components/TimePicker/TimePicker";
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

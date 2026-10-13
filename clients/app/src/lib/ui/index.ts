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
export { Chip, ChipRow, type ChipProps } from "./components/Chip/Chip";
export {
  CommandMenu,
  type Command,
  type CommandMenuProps,
} from "./components/CommandMenu/CommandMenu";
export { Composer, type ComposerProps } from "./components/Composer/Composer";
export {
  ConversationRow,
  type ConversationRowProps,
} from "./components/ConversationRow/ConversationRow";
export { Dot, type DotProps } from "./components/Dot/Dot";
export { EmptyState, type EmptyStateProps } from "./components/EmptyState/EmptyState";
export { Icon, ICON_NAMES, type IconName, type IconProps } from "./components/Icon/Icon";
export { IconButton, type IconButtonProps } from "./components/IconButton/IconButton";
export {
  Item,
  ItemDetail,
  SwipeAction,
  type ItemDetailProps,
  type ItemProps,
} from "./components/Item/Item";
export { Kbd, KbdHint, type KbdHintProps } from "./components/Kbd/Kbd";
export {
  Attachment,
  MessageCard,
  MetaLine,
  type MessageCardProps,
} from "./components/Message/Message";
export { NavItem, type NavItemProps } from "./components/NavItem/NavItem";
export {
  Note,
  type NoteChoice,
  type NoteChoiceOption,
  type NoteLink,
  type NoteProps,
} from "./components/Note/Note";
export { Pill, type PillProps } from "./components/Pill/Pill";
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
  Segmented,
  type SegmentedOption,
  type SegmentedProps,
} from "./components/Segmented/Segmented";
export {
  ListPane,
  PaneHeader,
  ReaderBody,
  ReaderPane,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  Spacer,
  StatusLine,
  WorkspaceSwitcher,
  type WorkspaceSwitcherProps,
} from "./components/Shell/Shell";
export { ShortcutSheet, type ShortcutSheetProps } from "./components/ShortcutSheet/ShortcutSheet";
export { Skeleton, SkeletonList, type SkeletonProps } from "./components/Skeleton/Skeleton";
export { TabBar, type TabBarItem, type TabBarProps } from "./components/TabBar/TabBar";
export { Tag, type TagProps } from "./components/Tag/Tag";
export {
  Toast,
  ToastRegion,
  UndoProvider,
  useUndoToast,
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

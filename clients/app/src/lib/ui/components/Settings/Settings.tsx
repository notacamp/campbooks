import { useId, type ReactNode } from "react";

import { cx } from "../../internal/cx";
import { InCardProvider } from "../../internal/in-card";
import { Icon } from "../Icon/Icon";
import { ReaderBody, ReaderPane } from "../Shell/Shell";
import { Heading } from "../Typography/Typography";

import styles from "./Settings.module.css";

export interface SettingsPageProps {
  /** The page's accessible name (its landmark). */
  label: string;
  /** The page title, in the display face. */
  title: string;
  /** One or two sentences under the title. */
  lead?: ReactNode | undefined;
  /** Buttons or a switch beside the title (they wrap under it on phones). */
  actions?: ReactNode | undefined;
  children: ReactNode;
}

/**
 * A settings page: the title and lead, then its sections as cards. The one
 * layout every Settings and Admin page shares, so none reads as a single
 * undifferentiated column.
 */
export function SettingsPage({ label, title, lead, actions, children }: SettingsPageProps) {
  return (
    <ReaderPane label={label}>
      <ReaderBody>
        <header className={styles["header"]}>
          <div className={styles["heading"]}>
            <Heading level={1} size="page">
              {title}
            </Heading>
            {lead === undefined ? null : <p className={styles["lead"]}>{lead}</p>}
          </div>
          {actions === undefined ? null : <div className={styles["actions"]}>{actions}</div>}
        </header>
        {children}
      </ReaderBody>
    </ReaderPane>
  );
}

export interface SettingsSectionProps {
  title: string;
  /** How many things the section lists, shown as "Title · 3". */
  count?: number | undefined;
  /** One line under the title. */
  description?: ReactNode | undefined;
  /** The title's id, for a group or form inside the section to be named by it. */
  titleId?: string | undefined;
  /** h2 on a page of sections; h3 for a section nested in another's content. */
  headingLevel?: 2 | 3 | undefined;
  className?: string | undefined;
  children: ReactNode;
}

/** A card with a title, an optional count and description, and its body. */
export function SettingsSection({
  title,
  count,
  description,
  titleId,
  headingLevel = 2,
  className,
  children,
}: SettingsSectionProps) {
  const generated = useId();
  const id = titleId ?? generated;
  const Tag = `h${String(headingLevel)}` as "h2" | "h3";
  return (
    <section className={cx(styles["section"], className)} aria-labelledby={id}>
      <div className={styles["top"]}>
        <Tag id={id} className={styles["title"]}>
          {title}
          {count === undefined ? null : <span className={styles["count"]}> · {count}</span>}
        </Tag>
        {description === undefined ? null : <p className={styles["description"]}>{description}</p>}
      </div>
      <InCardProvider value>{children}</InCardProvider>
    </section>
  );
}

export interface SettingsEmptyProps {
  /** The bold first line ("No passkeys yet."). */
  title: string;
  /** What to do about it. */
  children?: ReactNode | undefined;
  /** A button for it, at the end of the panel. */
  action?: ReactNode | undefined;
}

/** What a section says when it has nothing yet: a quiet panel, not a page-sized empty state. */
export function SettingsEmpty({ title, children, action }: SettingsEmptyProps) {
  return (
    <div className={styles["empty"]}>
      <p className={styles["emptyText"]}>
        <strong className={styles["emptyTitle"]}>{title}</strong>
        {children === undefined ? null : <> {children}</>}
      </p>
      {action}
    </div>
  );
}

/** A small tertiary line with a lock: what stays private. */
export function PrivacyLine({ children }: { children: ReactNode }) {
  return (
    <p className={styles["privacy"]}>
      <Icon name="lock" />
      <span>{children}</span>
    </p>
  );
}

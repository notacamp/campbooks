import type { MouseEventHandler } from "react";
import { useTranslation } from "react-i18next";

import { Icon, type IconName } from "../Icon/Icon";

import styles from "./TabBar.module.css";

export interface TabBarItem {
  id: string;
  label: string;
  icon: IconName;
  current?: boolean | undefined;
  /** A count on the tab (things that need you). */
  badge?: number | undefined;
  href?: string | undefined;
  onClick?: MouseEventHandler<HTMLElement> | undefined;
}

export interface TabBarProps {
  items: TabBarItem[];
  label?: string | undefined;
  /** Hide from the medium breakpoint up, where the sidebar takes over. */
  phoneOnly?: boolean | undefined;
}

/** The phone's bottom navigation: five places, 44px targets, safe-area aware. */
export function TabBar({ items, label, phoneOnly = false }: TabBarProps) {
  const { t } = useTranslation("ui");
  return (
    <nav
      className={styles["tabBar"]}
      aria-label={label ?? t("navigation.main")}
      data-phone-only={phoneOnly || undefined}
    >
      {items.map((item) => {
        const content = (
          <>
            <Icon name={item.icon} size="lg" />
            {item.label}
            {item.badge === undefined ? null : (
              <>
                {" "}
                <span className={styles["badge"]}>{item.badge}</span>
              </>
            )}
          </>
        );
        const shared = {
          className: styles["tab"],
          "aria-current": item.current === true ? ("page" as const) : undefined,
          onClick: item.onClick,
        };
        return item.href === undefined ? (
          <button key={item.id} {...shared} type="button">
            {content}
          </button>
        ) : (
          <a key={item.id} {...shared} href={item.href}>
            {content}
          </a>
        );
      })}
    </nav>
  );
}

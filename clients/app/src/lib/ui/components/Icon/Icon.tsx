import type { ReactElement } from "react";

import { cx } from "../../internal/cx";

import styles from "./Icon.module.css";

/* 24×24 line icons drawn with currentColor; stroke width is a token. */
const path = (d: string) => <path d={d} />;

const ICONS = {
  home: [path("M4 11.5 12 5l8 6.5V19a1 1 0 0 1-1 1h-4.5v-5h-5v5H5a1 1 0 0 1-1-1Z")],
  inbox: [
    path(
      "M3.5 13.5 6 5.5h12l2.5 8V18a1.5 1.5 0 0 1-1.5 1.5H5A1.5 1.5 0 0 1 3.5 18ZM3.5 13.5H8l1.5 2.5h5l1.5-2.5h4.5",
    ),
  ],
  schedule: [
    <rect key="r" x="3.5" y="5" width="17" height="15" rx="2.5" />,
    path("M3.5 10h17M8 3v4M16 3v4"),
  ],
  check: [path("m5 12.5 4.5 4.5L19 7.5")],
  document: [
    path("M6.5 3h7.2L19 8.3V19a2 2 0 0 1-2 2H6.5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z"),
    path("M13.5 3v5.5H19"),
  ],
  money: [
    <rect key="r" x="2.8" y="6" width="18.4" height="12" rx="3" />,
    <circle key="c" cx="12" cy="12" r="2.7" />,
  ],
  keyboard: [
    <rect key="r" x="3" y="6.5" width="18" height="11" rx="2" />,
    path("M7 10.5h.01M11 10.5h.01M15 10.5h.01M8 14h8"),
  ],
  warning: [
    path(
      "M12 8v5M12 16.5v.01M10.3 4.2 2.8 17.5A2 2 0 0 0 4.5 20.5h15a2 2 0 0 0 1.7-3L13.7 4.2a2 2 0 0 0-3.4 0Z",
    ),
  ],
  offline: [
    path(
      "M3 3l18 18M8.5 8.6A9 9 0 0 0 4 11.5M12 5.5a12 12 0 0 1 8 3M10 13.5a4 4 0 0 1 4.6.4M12 18.5h.01",
    ),
  ],
  "chevron-up": [path("m6 15 6-6 6 6")],
  "chevron-down": [path("m6 9 6 6 6-6")],
  "chevron-left": [path("m15 6-6 6 6 6")],
  clock: [<circle key="c" cx="12" cy="12" r="8.5" />, path("M12 7.2V12l3.2 2")],
  more: [
    <circle key="a" cx="5" cy="12" r="1.3" data-filled="" />,
    <circle key="b" cx="12" cy="12" r="1.3" data-filled="" />,
    <circle key="c" cx="19" cy="12" r="1.3" data-filled="" />,
  ],
  reply: [path("M9 14 4 9l5-5"), path("M4 9h10.5a5.5 5.5 0 0 1 0 11H11")],
  attachment: [
    path(
      "M20 11.5 12.3 19a5 5 0 0 1-7-7l8-8a3.3 3.3 0 0 1 4.7 4.7l-8 8a1.7 1.7 0 0 1-2.4-2.4l7.3-7.3",
    ),
  ],
  event: [path("M3.5 5h17v15h-17zM3.5 10h17M8 3v4M16 3v4")],
  settings: [
    <circle key="i" cx="12" cy="12" r="2.8" />,
    <circle key="o" cx="12" cy="12" r="6.5" />,
    path(
      "M12 2.8v2.7M12 18.5v2.7M2.8 12h2.7M18.5 12h2.7M5.5 5.5l1.9 1.9M16.6 16.6l1.9 1.9M5.5 18.5l1.9-1.9M16.6 7.4l1.9-1.9",
    ),
  ],
  people: [
    <circle key="a" cx="9" cy="8.5" r="3.2" />,
    path("M3.5 19.5a5.5 5.5 0 0 1 11 0"),
    path("M15.5 5.6a3 3 0 0 1 0 5.8M17.5 14.4a5.5 5.5 0 0 1 3 5.1"),
  ],
  plus: [path("M12 5v14M5 12h14")],
  "sign-out": [
    path("M10 4.5H6.5A1.5 1.5 0 0 0 5 6v12a1.5 1.5 0 0 0 1.5 1.5H10"),
    path("m15 8 4 4-4 4M19 12h-9"),
  ],
  mail: [<rect key="r" x="3.5" y="5.5" width="17" height="13" rx="2" />, path("m4 7 8 6 8-6")],
  "chevron-right": [path("m9 6 6 6-6 6")],
  close: [path("M6 6l12 12M18 6 6 18")],
  copy: [
    <rect key="r" x="8.5" y="8.5" width="11.5" height="11.5" rx="2" />,
    path("M15.5 8.5V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7.5a2 2 0 0 0 2 2h2.5"),
  ],
  "chevron-up-down": [path("m8 9.5 4-4 4 4M8 14.5l4 4 4-4")],
} satisfies Record<string, ReactElement[]>;

export type IconName = keyof typeof ICONS;

export const ICON_NAMES = Object.keys(ICONS) as IconName[];

export interface IconProps {
  name: IconName;
  size?: "md" | "lg" | undefined;
  className?: string | undefined;
}

/** A decorative line icon (always aria-hidden: pair it with visible text or a label). */
export function Icon({ name, size = "md", className }: IconProps) {
  return (
    <svg
      className={cx(styles["icon"], className)}
      data-size={size}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      {ICONS[name].map((element, index) => (
        <g key={index}>{element}</g>
      ))}
    </svg>
  );
}

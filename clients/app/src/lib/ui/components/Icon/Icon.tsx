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
  tasks: [<circle key="c" cx="12" cy="12" r="8.5" />, path("m8.5 12.3 2.4 2.4 4.6-5")],
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
  lock: [
    <rect key="r" x="5" y="11" width="14" height="9.5" rx="2" />,
    path("M8 11V7.5a4 4 0 0 1 8 0V11"),
  ],
  key: [<circle key="c" cx="8" cy="12" r="3.5" />, path("M11.5 12H20M17 12v3M20 12v2.5")],
  mail: [<rect key="r" x="3.5" y="5.5" width="17" height="13" rx="2" />, path("m4 7 8 6 8-6")],
  "chevron-right": [path("m9 6 6 6-6 6")],
  "arrow-right": [path("M4.5 12h15M13.5 6l6 6-6 6")],
  close: [path("M6 6l12 12M18 6 6 18")],
  copy: [
    <rect key="r" x="8.5" y="8.5" width="11.5" height="11.5" rx="2" />,
    path("M15.5 8.5V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v7.5a2 2 0 0 0 2 2h2.5"),
  ],
  "chevron-up-down": [path("m8 9.5 4-4 4 4M8 14.5l4 4 4-4")],
  send: [path("M4.5 12 20 4.5 15.5 20l-3.5-6.5Z"), path("M12 13.5 20 4.5")],
  bold: [path("M7.5 5h5.25a3.5 3.5 0 0 1 0 7H7.5Zm0 7h6.25a3.5 3.5 0 0 1 0 7H7.5Z")],
  italic: [path("M10.5 5H18M6 19h7.5M14.5 5l-5 14")],
  bullets: [
    path("M10 7h10M10 12h10M10 17h10"),
    <circle key="a" cx="5.5" cy="7" r="1" />,
    <circle key="b" cx="5.5" cy="12" r="1" />,
    <circle key="c" cx="5.5" cy="17" r="1" />,
  ],
  numbers: [
    path(
      "M10.5 7H20M10.5 12H20M10.5 17H20M4.5 5.5 6 4.5v5M4.5 14.5a1.5 1.5 0 1 1 2.5 1.1L4.5 19.5H7",
    ),
  ],
  star: [path("m12 4 2.4 5 5.4.7-4 3.7 1 5.4-4.8-2.6-4.8 2.6 1-5.4-4-3.7 5.4-.7Z")],
  "star-filled": [
    <path
      key="s"
      fill="currentColor"
      d="m12 4 2.4 5 5.4.7-4 3.7 1 5.4-4.8-2.6-4.8 2.6 1-5.4-4-3.7 5.4-.7Z"
    />,
  ],
  // Pin: the provider's star, named Pin in the UI.
  pin: [path("M9 4.5h6l-1 5 3 3v1.5H7V12.5l3-3Z"), path("M12 14v6")],
  "pin-filled": [
    <path key="p" fill="currentColor" d="M9 4.5h6l-1 5 3 3v1.5H7V12.5l3-3Z" />,
    path("M12 14v6"),
  ],
  archive: [
    <rect key="r" x="3.5" y="4.5" width="17" height="4.5" rx="1.5" />,
    path("M5 9v9a1.5 1.5 0 0 0 1.5 1.5h11A1.5 1.5 0 0 0 19 18V9M10 13h4"),
  ],
  move: [path("M4 12h11M11 8l4 4-4 4"), path("M19 5v14")],
  image: [
    <rect key="r" x="3.5" y="4.5" width="17" height="15" rx="2.5" />,
    <circle key="c" cx="9" cy="10" r="1.6" />,
    path("m4 17 5-4.5 4 3.5 2.5-2 4.5 3.5"),
  ],
  forward: [path("m15 6 5 5-5 5"), path("M20 11H9.5A5.5 5.5 0 0 0 4 16.5V19")],
  "reply-all": [
    path("M11 14 6 9l5-5"),
    path("M7 14 2 9l5-5"),
    path("M6 9h8.5a5.5 5.5 0 0 1 0 11H12"),
  ],
  unread: [<circle key="c" cx="12" cy="12" r="4" fill="currentColor" />],
  download: [path("M12 4v11M7.5 10.5 12 15l4.5-4.5"), path("M5 19.5h14")],
  search: [<circle key="c" cx="10.5" cy="10.5" r="6" />, path("m15 15 5 5")],
  preview: [
    path("M2.8 12S6.2 5.5 12 5.5 21.2 12 21.2 12 17.8 18.5 12 18.5 2.8 12 2.8 12Z"),
    <circle key="c" cx="12" cy="12" r="2.8" />,
  ],
  // Delete, and the Trash it moves mail to.
  trash: [
    path("M4.5 7h15M9.5 7V4.5h5V7"),
    path(
      "M6.5 7l.8 11.6A1.5 1.5 0 0 0 8.8 20h6.4a1.5 1.5 0 0 0 1.5-1.4L17.5 7M10.2 11v5M13.8 11v5",
    ),
  ],
  // The calendar's kinds: a flight, a bed, a ticket, a table, a parcel, a card, an hourglass, a done task.
  travel: [path("M21.5 2.5 11 13M21.5 2.5l-6.5 19-4-8.5-8.5-4Z")],
  stay: [
    path("M3.5 19V6M3.5 14h17v5M20.5 14a3 3 0 0 0-3-3h-7v3"),
    <circle key="c" cx="7" cy="10.5" r="1.5" />,
  ],
  ticket: [path("M4 6.5h16v4a2 2 0 0 0 0 4v4H4v-4a2 2 0 0 0 0-4ZM14.5 6.5v12")],
  dining: [path("M7 3v18M4 3v5a3 3 0 0 0 6 0V3M17 21V3c-2 1-3 4-3 7h3")],
  parcel: [path("M3.5 7 12 3l8.5 4v10L12 21l-8.5-4ZM3.5 7 12 11l8.5-4M12 11v10")],
  card: [<rect key="r" x="3" y="6" width="18" height="12" rx="2" />, path("M3 10h18M7 15h3")],
  hourglass: [path("M6.5 3h11M6.5 21h11M7.5 3c0 5 9 5 9 9s-9 4-9 9M16.5 3c0 5-9 5-9 9s9 4 9 9")],
  task: [<circle key="c" cx="12" cy="12" r="8.5" />, path("m8.8 12.2 2.3 2.3 4.2-4.6")],
  edit: [path("M4.5 19.5 5 15.5 15.5 5a2 2 0 0 1 3 3L8 18.5ZM13.5 7l3.5 3.5")],
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

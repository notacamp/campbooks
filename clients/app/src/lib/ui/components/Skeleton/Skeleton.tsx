import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";

import { VisuallyHidden } from "../Typography/Typography";

import styles from "./Skeleton.module.css";

export interface SkeletonProps {
  /** A line of text, or an avatar-sized circle. */
  shape?: "line" | "avatar" | undefined;
  /** Width of a line as a percentage of its container ("40%"). */
  width?: `${number}%` | undefined;
}

/** A shimmering placeholder. Decorative: wrap a group in {@link SkeletonList} for the announcement. */
export function Skeleton({ shape = "line", width = "100%" }: SkeletonProps) {
  return (
    <span
      className={styles["skeleton"]}
      data-shape={shape}
      style={{ "--skeleton-width": width } as CSSProperties}
      aria-hidden="true"
    />
  );
}

const ROW_WIDTHS: [`${number}%`, `${number}%`][] = [
  ["40%", "80%"],
  ["55%", "70%"],
  ["35%", "75%"],
];

/** Queue-shaped loading rows, announced once as "Loading". */
export function SkeletonList({ rows = 3 }: { rows?: number }) {
  const { t } = useTranslation("ui");
  return (
    <div className={styles["list"]} role="status" aria-busy="true">
      <VisuallyHidden>{t("loading")}</VisuallyHidden>
      {Array.from({ length: rows }, (_, index) => {
        const [first, second] = ROW_WIDTHS[index % ROW_WIDTHS.length] ?? ["50%", "70%"];
        return (
          <div key={index} className={styles["row"]}>
            <Skeleton shape="avatar" />
            <Skeleton width={first} />
            <Skeleton width={second} />
          </div>
        );
      })}
    </div>
  );
}

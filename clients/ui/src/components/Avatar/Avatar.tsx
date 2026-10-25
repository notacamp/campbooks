import { cx } from "../../internal/cx";

import styles from "./Avatar.module.css";

/** Number of identity hues in tokens.css (--avatar-1-h … --avatar-8-h). */
export const AVATAR_HUES = 8;

/** A stable hue (1–8) for a name: FNV-1a over the normalised name. */
export function avatarHue(name: string): number {
  let hash = 2_166_136_261; // FNV-1a offset basis
  for (const char of name.trim().toLowerCase()) {
    hash ^= char.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16_777_619); // FNV prime
  }
  return ((hash >>> 0) % AVATAR_HUES) + 1;
}

/** "Sofia Pessoa" → "SP", "Duarte & Pessoa" → "DP", "Cloudhost" → "CL". */
export function initialsOf(name: string): string {
  const words = name.split(/[\s·,&]+/u).filter((word) => /\p{L}/u.test(word));
  const [first = "", second] = words;
  const letters = second === undefined ? first.slice(0, 2) : (first[0] ?? "") + (second[0] ?? "");
  return letters.toLocaleUpperCase();
}

export interface AvatarProps {
  /** Who this is. Drives the initials and the identity hue. */
  name: string;
  /** Override the derived initials. */
  initials?: string | undefined;
  /** People are round; services (machines, bulk senders) are squircles. */
  kind?: "person" | "service" | undefined;
  size?: "sm" | "md" | "lg" | undefined;
  /** Decorative by default (the name is shown next to it); set to label it. */
  label?: string | undefined;
  className?: string | undefined;
}

export function Avatar({
  name,
  initials,
  kind = "person",
  size = "md",
  label,
  className,
}: AvatarProps) {
  return (
    <span
      className={cx(styles["avatar"], className)}
      data-avatar-hue={avatarHue(name)}
      data-kind={kind}
      data-size={size}
      {...(label === undefined ? { "aria-hidden": true } : { role: "img", "aria-label": label })}
    >
      {initials ?? initialsOf(name)}
    </span>
  );
}

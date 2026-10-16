import styles from "./ProviderMark.module.css";

export interface ProviderMarkProps {
  /** The provider's name ("Gmail", "Zoho Mail"): its first letter is drawn. */
  name: string;
  size?: "md" | "lg" | undefined;
}

/**
 * A neutral tile for a mail provider: its initial, in the theme's colours
 * (never a brand's). Decorative: always pair it with the provider's name.
 */
export function ProviderMark({ name, size = "md" }: ProviderMarkProps) {
  return (
    <span className={styles["mark"]} data-size={size} aria-hidden="true">
      {name.trim().charAt(0).toLocaleUpperCase()}
    </span>
  );
}

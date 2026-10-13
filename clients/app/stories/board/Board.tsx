import type { ReactNode } from "react";

import styles from "./Board.module.css";

/** A titled board of cells, like the States and Themes prototypes (stories only). */
export function Board({
  title,
  intro,
  children,
}: {
  title: string;
  intro?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className={styles["board"]}>
      <div className={styles["head"]}>
        <h1 className={styles["title"]}>{title}</h1>
        {intro === undefined ? null : <p className={styles["intro"]}>{intro}</p>}
      </div>
      {children}
    </div>
  );
}

export function BoardGrid({ children }: { children: ReactNode }) {
  return <div className={styles["grid"]}>{children}</div>;
}

export function BoardCell({
  title,
  children,
  lang,
}: {
  title: string;
  children: ReactNode;
  lang?: string;
}) {
  return (
    <section className={styles["cell"]} lang={lang} aria-label={title}>
      <h2 className={styles["cellTitle"]}>{title}</h2>
      {children}
    </section>
  );
}

/** A cell that carries its own theme and accent, like the Themes board. */
export function ThemeTile({
  title,
  theme,
  accent,
  children,
}: {
  title: string;
  theme: string;
  accent: string;
  children: ReactNode;
}) {
  return (
    <section
      className={styles["cell"]}
      data-theme={theme}
      data-accent={accent}
      data-tile=""
      aria-label={title}
    >
      <h2 className={styles["cellTitle"]}>{title}</h2>
      {children}
    </section>
  );
}

/** A small code block (token names). */
export function Code({ children }: { children: ReactNode }) {
  return <code className={styles["code"]}>{children}</code>;
}

import type { ReactNode } from "react";

import { type Signal, Dot, Heading, Visor, type VisorState } from "@notacamp/ui";

import styles from "./Queue.module.css";

export interface HomeLayoutProps {
  /** The queue: hero, then one group per signal. */
  children: ReactNode;
  /** Context next to the queue (today, money, waiting on others). */
  aside: ReactNode;
  asideLabel: string;
}

/** Home: the "needs you" queue with its context column (stacks on narrow screens). */
export function HomeLayout({ children, aside, asideLabel }: HomeLayoutProps) {
  return (
    <main className={styles["page"]}>
      <div className={styles["home"]}>
        <div className={styles["main"]}>{children}</div>
        <aside className={styles["aside"]} aria-label={asideLabel}>
          {aside}
        </aside>
      </div>
    </main>
  );
}

export interface PageHeroProps {
  title: string;
  /** Small caps line above the title (the date). */
  dateLine?: string | undefined;
  subline?: string | undefined;
  /** The visor's mood: alert when something needs review, happy when all clear. */
  visor?: VisorState | undefined;
  /** Smaller title and no visor, for phones. */
  compact?: boolean | undefined;
}

/** The page's one sentence: "6 things need you". */
export function PageHero({
  title,
  dateLine,
  subline,
  visor = "idle",
  compact = false,
}: PageHeroProps) {
  return (
    <header className={styles["hero"]} data-compact={compact || undefined}>
      {compact ? null : <Visor state={visor} size="xl" />}
      <div>
        {dateLine === undefined ? null : <p className={styles["dateLine"]}>{dateLine}</p>}
        <Heading level={1} size={compact ? "page" : "hero"}>
          {title}
        </Heading>
        {subline === undefined ? null : <p className={styles["subline"]}>{subline}</p>}
      </div>
    </header>
  );
}

export interface QueueGroupProps {
  /** The reason's signal colour, as a dot before the name; none for a band. */
  signal?: Signal | undefined;
  /** `danger`: the name in the danger colour (Now). */
  tone?: "danger" | undefined;
  name: string;
  /** How many items it holds; none for a group that holds only events (Next up, a Today of events). */
  count?: number | undefined;
  /** One action for the whole group, at the end of its heading ("All 3 already paid"). */
  action?: ReactNode | undefined;
  /** A muted line under the heading, saying why the group is as it is. */
  note?: string | undefined;
  /** {@link Item}s. */
  children: ReactNode;
}

/**
 * One group of the queue: a priority band (Now, Today, This week,
 * Whenever) or a reason (Reply, Pay, ...). Its heading gives the structure;
 * it is deliberately not a landmark (six regions per page would only add
 * noise for screen-reader users).
 */
export function QueueGroup({ signal, tone, name, count, action, note, children }: QueueGroupProps) {
  return (
    <section className={styles["group"]} data-signal={signal}>
      <div className={styles["groupHead"]}>
        <h2 className={styles["groupTitle"]} data-tone={tone}>
          {signal === undefined ? null : <Dot signal={signal} />}
          {name}
          {count === undefined ? null : (
            <>
              {" "}
              <span className={styles["count"]}>{count}</span>
            </>
          )}
        </h2>
        {action === undefined ? null : <div className={styles["groupAction"]}>{action}</div>}
      </div>
      {note === undefined ? null : <p className={styles["groupNote"]}>{note}</p>}
      <div className={styles["items"]}>{children}</div>
    </section>
  );
}

export interface AsideSectionProps {
  title: string;
  /** Drawn before the title (a Visor for what the assistant did). */
  leading?: ReactNode | undefined;
  children: ReactNode;
}

export function AsideSection({ title, leading, children }: AsideSectionProps) {
  return (
    <section className={styles["asideSection"]}>
      <h2 className={styles["asideTitle"]}>
        {leading}
        {title}
      </h2>
      {children}
    </section>
  );
}

export interface LineProps {
  /** "09:00": a fixed-width time column. */
  time?: string | undefined;
  /** Dot, avatar or visor before the text. */
  leading?: ReactNode | undefined;
  children: ReactNode;
  /** Muted detail or an action at the end. */
  trailing?: ReactNode | undefined;
  /** Struck through: an action that was undone. */
  undone?: boolean | undefined;
  href?: string | undefined;
}

/** One compact line of context (an event, a bill, something the assistant did). */
export function Line({ time, leading, children, trailing, undone = false, href }: LineProps) {
  const content = (
    <>
      {time === undefined ? null : <span className={styles["time"]}>{time}</span>}
      {leading}
      <span className={styles["lineText"]}>{children}</span>
      {trailing}
    </>
  );
  return href === undefined ? (
    <div className={styles["line"]} data-undone={undone || undefined}>
      {content}
    </div>
  ) : (
    <a className={styles["line"]} href={href}>
      {content}
    </a>
  );
}

/** Muted trailing text for a {@link Line}. */
export function LineMeta({ children }: { children: ReactNode }) {
  return <span className={styles["lineMeta"]}>{children}</span>;
}

/** A big tabular number with its label (money owed, money coming in). */
export function Figure({ value, label }: { value: string; label: string }) {
  return (
    <div>
      <div className={styles["figure"]}>{value}</div>
      <div className={styles["figureLabel"]}>{label}</div>
    </div>
  );
}

export function FigureRow({ children }: { children: ReactNode }) {
  return <div className={styles["figureRow"]}>{children}</div>;
}

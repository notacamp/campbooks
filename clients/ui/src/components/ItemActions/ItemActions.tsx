import { useEffect, useRef } from "react";

import type { ShortcutId } from "../../shortcuts";

import { Button } from "../Button/Button";
import { Visor } from "../Visor/Visor";

import styles from "./ItemActions.module.css";

/** One thing the member can do with an item, in words, with its key. */
export interface ItemAction {
  /** Stable id the screen acts on ("reply", "done", a server action's id). */
  id: string;
  /** What it does: "Reply", "Snooze…", "Say yes and add to calendar". */
  label: string;
  /** It runs the AI: the visor goes before the label. */
  ai?: boolean | undefined;
  /** Its shortcut in the registry (`item.choice1`, `item.done`, `item.snooze`). */
  key: ShortcutId;
}

export interface ItemActionsProps {
  /** The item's own actions, best first (keys 1 to 3). */
  own: readonly ItemAction[];
  /**
   * After the own actions, in the same style, with keys of their own
   * (Continue with Scout, `.`): never one of 1 to 3.
   */
  after?: readonly ItemAction[] | undefined;
  /** Done, Snooze, Not needed: always there, after a thin divider. */
  universal: readonly ItemAction[];
  /** The action the arrow keys are on; focus follows it. */
  highlighted?: string | undefined;
  /**
   * A button was focused (Tab, a click): the highlight follows the focus;
   * `undefined` once the focus leaves the group.
   */
  onHighlight?: ((id?: string) => void) | undefined;
  onAction: (id: string) => void;
  /** Accessible name of the group: "Actions for Marta Ferreira". */
  label: string;
  /**
   * The own actions were picked by the assistant for this item: the visor
   * goes before them, saying so ("Scout picked these").
   */
  picked?: string | undefined;
}

/**
 * The selected item's actions, all in one button style (label and key), in
 * two groups split by a divider; the visor before the own ones when the
 * assistant picked them. Roving tabindex: one button is in the tab
 * order (the highlighted one, else the first); ←/→ move the highlight from
 * the screen's shortcuts and the focus follows it.
 */
export function ItemActions({
  own,
  after = [],
  universal,
  highlighted,
  onHighlight,
  onAction,
  label,
  picked,
}: ItemActionsProps) {
  const group = useRef<HTMLDivElement>(null);
  const all = [...own, ...after, ...universal];
  const current = all.some((action) => action.id === highlighted) ? highlighted : undefined;
  const tabStop = current ?? all[0]?.id;

  useEffect(() => {
    if (current === undefined) {
      return;
    }
    const button = group.current?.querySelector<HTMLElement>(
      `[data-action="${CSS.escape(current)}"]`,
    );
    if (button !== null && button !== undefined && document.activeElement !== button) {
      button.focus({ preventScroll: true });
    }
  }, [current]);

  const render = (action: ItemAction) => (
    <Button
      key={action.id}
      variant="secondary"
      ai={action.ai}
      shortcut={action.key}
      data-action={action.id}
      data-highlighted={action.id === current || undefined}
      tabIndex={action.id === tabStop ? 0 : -1}
      onFocus={() => {
        if (action.id !== current) {
          onHighlight?.(action.id);
        }
      }}
      onBlur={(event) => {
        // The focus left the group: Enter there must not press a highlighted action.
        const next = event.relatedTarget;
        if (!(next instanceof Node && group.current?.contains(next) === true)) {
          onHighlight?.();
        }
      }}
      onClick={() => {
        onAction(action.id);
      }}
    >
      {action.label}
    </Button>
  );

  return (
    <div ref={group} className={styles["actions"]} role="group" aria-label={label}>
      {picked !== undefined && own.length > 0 ? (
        <span className={styles["picked"]} role="img" aria-label={picked} title={picked}>
          <Visor size="xs" />
        </span>
      ) : null}
      {own.map(render)}
      {after.map(render)}
      {own.length + after.length > 0 && universal.length > 0 ? (
        <span className={styles["divider"]} aria-hidden="true" />
      ) : null}
      {universal.map(render)}
    </div>
  );
}

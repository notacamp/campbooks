import { useState, type DragEvent, type ReactNode } from "react";

import { Icon, Popover } from "@notacamp/ui";

import { MenuItem, MenuList } from "./Menu";
import { OptionTag, type OptionTagColor } from "./Cells";
import styles from "./Board.module.css";

export interface BoardGroup {
  id: string;
  name: string;
  color: OptionTagColor;
  /** How many rows the group holds in all. */
  count: number;
  /** A figure over its cards ("€14,400"), when every card is loaded. */
  summary?: string | undefined;
  /** Hidden groups are not drawn. */
  cards: readonly BoardCard[];
}

export interface BoardCard {
  id: string;
  title: string;
  /** The card's properties, drawn by the screen. */
  body: ReactNode;
  /** Open the row. */
  onOpen: () => void;
}

export interface BoardProps {
  /** Names the board ("Clients by stage"). */
  label: string;
  groups: readonly BoardGroup[];
  /** Move a card to a group (`null` is the group with no value). */
  onMove: (cardId: string, groupId: string) => void;
  /** The "Move to" menu's name for a card ("Move Acme Studio to"). */
  moveLabel: (title: string) => string;
  /** The "Move to" button's text and the menu's title. */
  moveText: string;
  /** Says how many cards are not loaded ("3 more"). */
  more?: ReactNode | undefined;
  /** A card the member may not move (a reader). */
  readOnly?: boolean | undefined;
  /** Add a card to a group. */
  onAdd?: ((groupId: string) => void) | undefined;
  addLabel?: ((group: string) => string) | undefined;
}

const DRAG_TYPE = "application/x-campbooks-card";

/**
 * Cards in a column per group. A card is dragged to another column, or moved
 * with its "Move to" menu (the keyboard way). Each column holds its own
 * cards; the board scrolls sideways inside its box.
 */
export function Board({
  label,
  groups,
  onMove,
  moveLabel,
  moveText,
  more,
  readOnly = false,
  onAdd,
  addLabel,
}: BoardProps) {
  const [dragging, setDragging] = useState<string | undefined>();
  const [over, setOver] = useState<string | undefined>();

  const drop = (event: DragEvent<HTMLElement>, group: BoardGroup) => {
    event.preventDefault();
    const id = event.dataTransfer.getData(DRAG_TYPE);
    setDragging(undefined);
    setOver(undefined);
    if (id !== "" && !group.cards.some((card) => card.id === id)) {
      onMove(id, group.id);
    }
  };

  return (
    <div className={styles["scroller"]}>
      <div className={styles["board"]} role="group" aria-label={label}>
        {groups.map((group) => (
          <div
            key={group.id}
            role="presentation"
            className={styles["column"]}
            data-over={over === group.id || undefined}
            onDragOver={(event) => {
              if (dragging !== undefined) {
                event.preventDefault();
                setOver(group.id);
              }
            }}
            onDragLeave={() => {
              setOver((current) => (current === group.id ? undefined : current));
            }}
            onDrop={(event) => {
              drop(event, group);
            }}
          >
            <div className={styles["head"]}>
              <OptionTag color={group.color}>{group.name}</OptionTag>
              <span className={styles["count"]}>{group.count}</span>
              {group.summary === undefined ? null : (
                <span className={styles["summary"]}>{group.summary}</span>
              )}
            </div>
            <ul className={styles["cards"]} aria-label={group.name}>
              {group.cards.map((card) => (
                <li key={card.id} className={styles["card"]}>
                  <div
                    role="presentation"
                    className={styles["draggable"]}
                    data-dragging={dragging === card.id || undefined}
                    draggable={!readOnly}
                    onDragStart={(event) => {
                      event.dataTransfer.effectAllowed = "move";
                      event.dataTransfer.setData(DRAG_TYPE, card.id);
                      setDragging(card.id);
                    }}
                    onDragEnd={() => {
                      setDragging(undefined);
                      setOver(undefined);
                    }}
                  >
                    <button type="button" className={styles["open"]} onClick={card.onOpen}>
                      <span className={styles["title"]}>{card.title}</span>
                      {card.body}
                    </button>
                  </div>
                  {readOnly ? null : (
                    <Popover
                      label={moveLabel(card.title)}
                      placement="bottom-end"
                      layer="top"
                      renderTrigger={(trigger) => (
                        <button
                          {...trigger}
                          type="button"
                          className={styles["move"]}
                          aria-label={moveLabel(card.title)}
                          title={moveText}
                        >
                          <Icon name="move" />
                        </button>
                      )}
                    >
                      {(close) => (
                        <MenuList label={moveLabel(card.title)}>
                          {groups.map((target) => (
                            <MenuItem
                              key={target.id}
                              checked={target.id === group.id}
                              onSelect={() => {
                                close();
                                if (target.id !== group.id) {
                                  onMove(card.id, target.id);
                                }
                              }}
                            >
                              {target.name}
                            </MenuItem>
                          ))}
                        </MenuList>
                      )}
                    </Popover>
                  )}
                </li>
              ))}
            </ul>
            {onAdd === undefined || readOnly ? null : (
              <button
                type="button"
                className={styles["add"]}
                aria-label={addLabel?.(group.name)}
                onClick={() => {
                  onAdd(group.id);
                }}
              >
                <Icon name="plus" />
              </button>
            )}
          </div>
        ))}
      </div>
      {more === undefined ? null : <div className={styles["more"]}>{more}</div>}
    </div>
  );
}

import type { KeyboardEvent } from "react";
import { useTranslation } from "react-i18next";

import { Button, Popover, PopoverSection } from "~/lib/ui";

import { STREAMS, type Stream } from "../../api/queries";
import { streamName, type Translate } from "../../model";

export interface MovePickerProps {
  current: Stream;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onMove: (stream: Stream) => void;
}

/** "Move" (M): the four streams, picked with a click or 1–4. */
export function MovePicker({ current, open, onOpenChange, onMove }: MovePickerProps) {
  const { t: typed } = useTranslation("conversations");
  const t = typed as unknown as Translate;
  const pick = (stream: Stream, close: () => void) => {
    close();
    onMove(stream);
  };
  return (
    <Popover
      label={t("actions.moveTo")}
      open={open}
      onOpenChange={onOpenChange}
      renderTrigger={(trigger) => (
        <Button {...trigger} variant="ghost" icon="move" shortcut="conversations.move_to_stream">
          {/* Icon-only on phones, still named for screen readers. */}
          <span className="max-md:sr-only">{t("actions.move")}</span>
        </Button>
      )}
    >
      {(close) => (
        <PopoverSection title={t("actions.moveTo")}>
          <div className="flex flex-col gap-0-5">
            {STREAMS.map((stream, index) => (
              <Button
                key={stream}
                variant="ghost"
                keys={String(index + 1)}
                disabled={stream === current}
                aria-current={stream === current ? "true" : undefined}
                onClick={() => {
                  pick(stream, close);
                }}
                // 1–4 pick a stream while the menu has focus; the keys stay here.
                onKeyDown={(event: KeyboardEvent<HTMLButtonElement>) => {
                  const target = STREAMS[Number(event.key) - 1];
                  if (target !== undefined && !event.metaKey && !event.ctrlKey && !event.altKey) {
                    event.preventDefault();
                    event.stopPropagation();
                    if (target !== current) {
                      pick(target, close);
                    }
                  }
                }}
              >
                {streamName(t, stream)}
              </Button>
            ))}
          </div>
        </PopoverSection>
      )}
    </Popover>
  );
}

import { ConversationRow, Highlight } from "~/lib/ui";
import { displayName, isStream, listTime, type Stream } from "~/modules/conversations";

import type { SearchHit } from "./api/queries";
import type { Translate } from "./model";

/** The stream a hit opens in: its own, or Primary when the server did not say. */
export function hitStream(hit: SearchHit): Stream {
  const stream = hit.facets.stream ?? "";
  return isStream(stream) ? stream : "primary";
}

export interface HitRowProps {
  hit: SearchHit;
  t: Translate;
  language: string;
  id?: string | undefined;
  /** The live flags, when the conversation's own summary is known (they win over the hit's). */
  unread?: boolean | undefined;
  starred?: boolean | undefined;
  current?: boolean | undefined;
  selected?: boolean | undefined;
  option?: boolean | undefined;
  now?: Date | undefined;
  onSelect: () => void;
}

/** A hit as an inbox row with the words that matched marked. */
export function HitRow({
  hit,
  t,
  language,
  id,
  unread,
  starred,
  current,
  selected,
  option,
  now,
  onSelect,
}: HitRowProps) {
  const stream = hit.facets.stream;
  const noSubject = hit.title.every((segment) => segment.text.trim() === "");
  return (
    <ConversationRow
      id={id}
      from={hit.from === null || hit.from === undefined ? "" : displayName(hit.from)}
      avatar={{
        kind:
          stream === undefined || stream === null || stream === "primary" ? "person" : "service",
      }}
      time={listTime(hit.occurred_at, language, now)}
      dateTime={hit.occurred_at}
      subject={noSubject ? t("row.noSubject") : <Highlight segments={hit.title} />}
      snippet={<Highlight segments={hit.snippet} />}
      unread={unread ?? hit.facets.unread ?? false}
      starred={starred ?? hit.facets.starred ?? false}
      count={hit.facets.count}
      attachment={hit.facets.attachment ?? false}
      current={current}
      selected={selected}
      option={option}
      onSelect={onSelect}
    />
  );
}

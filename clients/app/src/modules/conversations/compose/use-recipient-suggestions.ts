import { useState } from "react";

import { useDebounced } from "~/lib/hooks/use-debounced";
import type { Recipient, RecipientFieldName, RecipientSuggester } from "~/lib/ui";

import { useCorrespondents, type Correspondent } from "../api/queries";

/** How long typing settles before the server is asked. */
const DEBOUNCE_MS = 200;
/** Shortest text the server searches (`list_correspondents`). */
const MIN_CHARS = 2;

/** Lower-cased, without accents. */
function fold(text: string): string {
  return text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** Whether every word typed is in the person's name or address, as the server matches. */
function matches(person: Correspondent, typed: string): boolean {
  const haystack = fold(`${person.address} ${person.name ?? ""}`);
  return fold(typed)
    .split(/[\s,]+/u)
    .filter((word) => word !== "")
    .every((word) => haystack.includes(word));
}

/** The address fields of a message, as they stand. */
interface AddressFields {
  to: readonly Recipient[];
  cc: readonly Recipient[];
  bcc: readonly Recipient[];
}

/**
 * The composer's suggestions in To, Cc and Bcc: what the field being typed
 * in holds, once typing settles, asked of `conversations.list_correspondents`,
 * leaving out everyone already in any of the three fields. One field is
 * typed in at a time, so one question is asked at a time.
 */
export function useRecipientSuggestions(
  workspaceId: string,
  fields: AddressFields,
): RecipientSuggester {
  const [typing, setTyping] = useState<{ field: RecipientFieldName; text: string }>({
    field: "to",
    text: "",
  });
  const text = typing.text.trim();
  const settled = useDebounced(text, DEBOUNCE_MS);
  const found = useCorrespondents(workspaceId, settled, { enabled: settled.length >= MIN_CHARS });

  const taken = new Set(
    [...fields.to, ...fields.cc, ...fields.bcc].map((recipient) => recipient.address.toLowerCase()),
  );
  // Until the server answers for this very text, the previous answer stays,
  // filtered as the server would.
  const stale = settled !== text || found.isPlaceholderData;
  const people =
    text.length < MIN_CHARS
      ? []
      : (found.data ?? []).filter(
          (person) => !taken.has(person.address.toLowerCase()) && (!stale || matches(person, text)),
        );
  const loading = text.length >= MIN_CHARS && (settled !== text || found.isFetching);

  return {
    suggestions: (field) => (field === typing.field ? people : []),
    loading: (field) => field === typing.field && loading,
    onQueryChange: (field, value) => {
      // A field emptied after another took over says nothing about the other.
      setTyping((current) =>
        value === "" && current.field !== field ? current : { field, text: value },
      );
    },
  };
}

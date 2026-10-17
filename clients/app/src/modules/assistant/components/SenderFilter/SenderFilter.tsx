import { useState } from "react";
import { useTranslation } from "react-i18next";

import { Avatar, CheckCards, SearchField, Text, type CheckCardOption } from "~/lib/ui";
import { useDebounced } from "~/lib/hooks/use-debounced";

import { useSenders, type Sender } from "../../api/queries";
import { readSender } from "../../model";

/** How long typing settles before the server is asked. */
const DEBOUNCE_MS = 250;
/** Shortest text the server searches (`list_senders`). */
const MIN_CHARS = 2;

export interface SenderFilterProps {
  workspaceId: string;
  /** The group's legend, for screen readers (a heading already asks). */
  label: string;
  /** The ticked addresses or domains. */
  value: readonly string[];
  /** The new ticks, with the sender just ticked when it came from the mail. */
  onChange: (value: string[], ticked?: Sender) => void;
  /** Senders already seen, by address, so ticked ones keep their names. */
  known: ReadonlyMap<string, Sender>;
  /** Before anything is typed, list the senders replied to most. */
  suggest?: boolean | undefined;
}

/** Lower-cased, without accents. */
function fold(text: string): string {
  return text
    .normalize("NFD")
    .replaceAll(/\p{Diacritic}/gu, "")
    .toLowerCase();
}

/** Whether every word of `typed` is in the sender's address or name, as the server matches. */
function matches(sender: Sender, typed: string): boolean {
  const haystack = fold(`${sender.address} ${sender.name ?? ""}`);
  return fold(typed)
    .split(/[\s,]+/u)
    .filter((word) => word !== "")
    .every((word) => haystack.includes(word));
}

/**
 * Senders to tick, found by name: a filter box over the cards. Typing filters
 * what is shown at once and, from two characters, asks
 * `conversations.list_senders` with `q`, so anyone who writes to you can be
 * found without knowing their address. Ticked senders stay listed. A full
 * address or a domain no one matches becomes its own card; a free mail
 * domain says why it can't. In the box, Enter ticks the first match and
 * clears it, Esc clears it; with the box empty they reach the page.
 */
export function SenderFilter({
  workspaceId,
  label,
  value,
  onChange,
  known,
  suggest = false,
}: SenderFilterProps) {
  const { t } = useTranslation("assistant");
  const [draft, setDraft] = useState("");
  const typed = draft.trim();
  const settled = useDebounced(typed, DEBOUNCE_MS);
  const searching = settled.length >= MIN_CHARS;
  const top = useSenders(workspaceId, "", { enabled: suggest });
  const found = useSenders(workspaceId, settled, { enabled: searching });

  // What the server found for the settled text, or, until it answers, what
  // is already here, filtered as it would be.
  const pool = new Map<string, Sender>();
  for (const sender of [...(top.data ?? []), ...known.values()]) {
    pool.set(sender.address, sender);
  }
  const answered = searching && settled === typed && found.data !== undefined;
  const listed: Sender[] =
    typed === ""
      ? suggest
        ? (top.data ?? [])
        : []
      : answered
        ? found.data
        : [...pool.values()].filter((sender) => matches(sender, typed));
  const listedAddresses = new Set(listed.map((sender) => sender.address));

  const asSender = readSender(typed);
  const newOne =
    (asSender.kind === "address" || asSender.kind === "domain") &&
    !listedAddresses.has(asSender.value) &&
    !value.includes(asSender.value)
      ? asSender
      : undefined;
  const looking = typed.length >= MIN_CHARS && !answered && !found.isError;

  const card = (sender: Sender): CheckCardOption<string> => ({
    value: sender.address,
    title: sender.name ?? sender.address,
    description: sender.name === null || sender.name === undefined ? undefined : sender.address,
    meta: t("flow.always.count", { count: sender.received }),
    leading: <Avatar name={sender.name ?? sender.address} />,
  });
  const ticked = value
    .filter((each) => !listedAddresses.has(each))
    .map((each) => {
      const sender = known.get(each) ?? pool.get(each);
      return sender === undefined ? { value: each, title: each } : card(sender);
    });
  const options: CheckCardOption<string>[] = [
    ...ticked,
    ...listed.map(card),
    ...(newOne === undefined
      ? []
      : [
          {
            value: newOne.value,
            title: newOne.value,
            description:
              newOne.kind === "address"
                ? t("flow.find.newAddress")
                : t("flow.find.newDomain", { domain: newOne.value }),
          },
        ]),
  ];
  const byAddress = new Map(listed.map((sender) => [sender.address, sender]));

  const tick = (next: string[]) => {
    const added = next.find((each) => !value.includes(each));
    onChange(next, added === undefined ? undefined : byAddress.get(added));
  };
  /** Enter in the box: the first match not yet ticked, else the typed address. */
  const tickFirst = () => {
    const first = listed.find((sender) => !value.includes(sender.address));
    const pick = first?.address ?? newOne?.value;
    if (pick !== undefined) {
      onChange([...value, pick], first);
      setDraft("");
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <SearchField
        label={t("flow.find.label")}
        placeholder={t("flow.find.placeholder")}
        value={draft}
        onValueChange={setDraft}
        onClear={() => {
          setDraft("");
        }}
        inputMode="search"
        onKeyDown={(event) => {
          // With text in the box, Enter and Esc belong to it, not the page.
          if (typed === "") {
            return;
          }
          if (event.key === "Enter") {
            event.preventDefault();
            tickFirst();
          } else if (event.key === "Escape") {
            event.preventDefault();
            setDraft("");
          }
        }}
      />
      {options.length === 0 ? null : (
        <CheckCards label={label} hideLabel value={value} onChange={tick} options={options} />
      )}
      <div role="status">
        {asSender.kind === "free_mail" ? (
          <Text tone="secondary">{t("flow.add.freeMail", { domain: asSender.value })}</Text>
        ) : typed !== "" && listed.length === 0 && newOne === undefined && !looking ? (
          <Text tone="secondary">
            {found.isError ? t("flow.find.failed") : t("flow.find.none", { text: typed })}
          </Text>
        ) : suggest && typed === "" && top.data?.length === 0 ? (
          <Text tone="secondary">{t("flow.always.noSenders")}</Text>
        ) : null}
      </div>
    </div>
  );
}

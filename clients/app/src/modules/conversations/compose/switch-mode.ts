import type { Draft } from "../api/drafts";
import type { MessageView } from "../api/queries";

type Address = Draft["to"][number];

/** Who a draft is addressed to. */
export interface Recipients {
  to: Address[];
  cc: Address[];
  bcc: Address[];
}

const key = (address: Address) => address.address.trim().toLowerCase();

/**
 * The recipients the server fills in for a reply of `kind` to `message`
 * (`create_draft`'s `fill`, campbooks-cloud#394), so a switch can tell what
 * the member changed: a reply goes to the sender (or Reply-To, or the same
 * people again for mail you sent), a reply all adds To and Cc, and neither
 * names the inbox's own address. A forward starts with nobody.
 */
export function filledFor(
  kind: Draft["kind"],
  message: Pick<MessageView, "direction" | "from" | "to" | "cc" | "reply_to">,
  inboxAddress: string,
): Recipients {
  if (kind !== "reply" && kind !== "reply_all") {
    return { to: [], cc: [], bcc: [] };
  }
  const outbound = message.direction === "outbound";
  const to = outbound
    ? [...message.to]
    : message.reply_to.length === 0
      ? [message.from]
      : [...message.reply_to];
  const cc: Address[] = [];
  if (kind === "reply_all") {
    if (!outbound) {
      to.push(...message.to);
    }
    cc.push(...message.cc);
  }
  const ours = inboxAddress.trim().toLowerCase();
  const others = (list: Address[]) => list.filter((a) => key(a) !== ours);
  return { to: others(to), cc: others(cc), bcc: [] };
}

/**
 * The recipients of the new kind with the member's own changes kept: the
 * people they added stay in the field they put them in, the people they
 * removed stay out, the people in Bcc stay only there, and the rest is what
 * the new kind fills in.
 */
export function carryRecipients(
  now: Recipients,
  filledBefore: Recipients,
  filledAfter: Recipients,
): Recipients {
  const had = new Set([...filledBefore.to, ...filledBefore.cc].map(key));
  const has = new Set([...now.to, ...now.cc, ...now.bcc].map(key));
  const removed = new Set([...had].filter((k) => !has.has(k)));
  const seen = new Set<string>();
  const take = (list: Address[]) =>
    list.filter((a) => {
      const k = key(a);
      if (removed.has(k) || seen.has(k)) {
        return false;
      }
      seen.add(k);
      return true;
    });
  const added = (list: Address[]) => list.filter((a) => !had.has(key(a)));
  // Someone the member put in Bcc stays hidden there, whatever the new kind
  // fills in; then the new kind's own, so someone in both keeps its field.
  const bcc = take([...now.bcc]);
  const to = take([...filledAfter.to, ...added(now.to)]);
  const cc = take([...filledAfter.cc, ...added(now.cc)]);
  return { to, cc, bcc };
}

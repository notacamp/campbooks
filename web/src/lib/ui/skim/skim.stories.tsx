/**
 * Skim stories — CSF3 format (Storybook 10).
 *
 * Four stories:
 *   1. TodayDeck  — reply / pay / decide, draft on the first card
 *   2. InboxTriage — Keep / Archive
 *   3. SingleCard — one card
 *   4. ImmediatelyCleared — opens straight to the "cleared" screen
 *
 * Stories use locally-typed meta so the file compiles without
 * @storybook/react in package.json.
 */
import { useState, type JSX } from "react";
import { Skim, type SkimCard, type SkimProps } from "./skim";

// ── CSF3 meta ────────────────────────────────────────────────────────────────

const meta = {
  title: "ui/Skim",
  component: Skim,
  tags: ["autodocs"],
  parameters: { layout: "padded" },
};

export default meta;

type Story = {
  name?: string;
  render?: () => JSX.Element;
  args?: Partial<SkimProps>;
};

// ── Interactive wrapper ───────────────────────────────────────────────────────

/**
 * Wraps the Skim component with the open/close toggle state so every story
 * is interactive out-of-the-box in Storybook Canvas.
 */
const SkimStory = ({
  cards,
  clearedTitle,
  clearedSub,
}: Pick<SkimProps, "cards" | "clearedTitle" | "clearedSub">): JSX.Element => {
  const [open, setOpen] = useState(false);

  const handleAction = (_card: SkimCard, _key: string): void => {
    // Action is a no-op in stories; callers perform the real mutation.
  };

  return (
    // position:relative gives Skim its positioned ancestor
    <div
      style={{
        position: "relative",
        width: "100%",
        height: "640px",
        background: "var(--cb-bg)",
        borderRadius: "var(--cb-r4)",
        overflow: "hidden",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {!open && (
        <button
          type="button"
          onClick={() => {
            setOpen(true);
          }}
          style={{
            padding: "10px 20px",
            borderRadius: "var(--cb-r2)",
            background: "var(--cb-accent)",
            color: "var(--cb-on-accent)",
            border: "none",
            cursor: "pointer",
            font: "600 14px inherit",
          }}
        >
          Open Skim
        </button>
      )}

      <Skim
        open={open}
        cards={cards}
        clearedTitle={clearedTitle}
        clearedSub={clearedSub}
        onAction={handleAction}
        onClose={() => {
          setOpen(false);
        }}
      />
    </div>
  );
}

// ── Deck definitions ──────────────────────────────────────────────────────────

const todayCards: SkimCard[] = [
  {
    id: "reply-sofia",
    avatar: { initials: "SP", place: "people", kind: "person" },
    title: "Reply to Sofia Pessoa",
    subtitle: "Brightloop · Q4 proposal",
    pill: { place: "people", label: "Reply" },
    read: (
      <>
        She needs your review of the pricing before{" "}
        <strong>Thursday</strong> — she sends it to her board on Friday.
      </>
    ),
    draft: (
      <>
        Hi Sofia, thanks for this. The revised scope reads well and the
        pricing looks right to me, with one question on the retainer
        terms…
      </>
    ),
    actions: [
      { key: "send-draft", label: "Send draft", kind: "primary", dir: "right" },
      { key: "hold-time", label: "Hold time", kind: "secondary", dir: "down" },
      { key: "not-now", label: "Not now", kind: "ghost", dir: "left" },
    ],
  },
  {
    id: "pay-cloudhost",
    avatar: { initials: "CH", place: "money", kind: "service" },
    title: "Pay Cloudhost",
    subtitle: "Invoice CH-20418 · €148.00",
    pill: { place: "money", label: "Pay" },
    read: (
      <>
        Due <strong>Friday</strong>. It&rsquo;s not on a statement yet, so
        I can&rsquo;t tell if it&rsquo;s already paid.
      </>
    ),
    actions: [
      { key: "paid", label: "I've paid it", kind: "primary", dir: "right" },
      {
        key: "remind-thu",
        label: "Remind Thursday",
        kind: "secondary",
        dir: "down",
      },
      { key: "not-now", label: "Not now", kind: "ghost", dir: "left" },
    ],
  },
  {
    id: "decide-marta",
    avatar: { initials: "MD", place: "time", kind: "person" },
    title: "Decide a date for Marta",
    subtitle: "Office move",
    pill: { place: "time", label: "Decide" },
    read: (
      <>
        She needs a date. You&rsquo;re free{" "}
        <strong>Tuesday 14:00–16:00</strong> — I can hold it and draft
        the confirmation.
      </>
    ),
    actions: [
      {
        key: "hold-tue",
        label: "Hold Tue 14:00",
        kind: "primary",
        dir: "right",
      },
      { key: "pick-other", label: "Pick another", kind: "secondary", dir: "down" },
      { key: "not-now", label: "Not now", kind: "ghost", dir: "left" },
    ],
  },
];

const inboxCards: SkimCard[] = [
  {
    id: "newsletter-dw",
    avatar: { initials: "DS", place: "now", kind: "service" },
    title: "Designer Weekly",
    subtitle: "Newsletter · Friday",
    pill: { place: "none", label: "FYI" },
    read: "12 portfolios this month. No action needed unless you want a read later.",
    actions: [
      { key: "keep", label: "Keep", kind: "secondary", dir: "right" },
      { key: "archive", label: "Archive", kind: "ghost", dir: "left" },
    ],
  },
  {
    id: "pedro-nunes",
    avatar: { initials: "PN", place: "paper", kind: "person" },
    title: "Pedro Nunes",
    subtitle: "Sunday",
    pill: { place: "none", label: "FYI" },
    read: '"Thanks, payment received. Talk soon." Nothing owed either way.',
    actions: [
      { key: "keep", label: "Keep", kind: "secondary", dir: "right" },
      { key: "archive", label: "Archive", kind: "ghost", dir: "left" },
    ],
  },
  {
    id: "voltio-invoice",
    avatar: { initials: "VE", place: "paper", kind: "service" },
    title: "Voltio Energia",
    subtitle: "September invoice · €86.12",
    pill: { place: "money", label: "Invoice" },
    read: "I filed this to Books already. Approve it here, or leave it for the reconciliation.",
    actions: [
      { key: "approve", label: "Approve", kind: "primary", dir: "right" },
      { key: "later", label: "Later", kind: "ghost", dir: "left" },
    ],
  },
  {
    id: "joana-silva",
    avatar: { initials: "JS", place: "people", kind: "person" },
    title: "Joana Silva",
    subtitle: "Friday",
    pill: { place: "none", label: "Optional" },
    read: '"Loved the moodboard. One thought on the type." A friendly reply, no deadline.',
    actions: [
      { key: "reply", label: "Reply", kind: "primary", dir: "right" },
      { key: "keep", label: "Keep", kind: "secondary", dir: "down" },
      { key: "archive", label: "Archive", kind: "ghost", dir: "left" },
    ],
  },
];

const singleCard: SkimCard[] = [
  {
    id: "single-rui",
    avatar: { initials: "RC", place: "people", kind: "person" },
    title: "Reply to Rui Costa",
    subtitle: "Which logo files to send",
    pill: { place: "people", label: "Reply" },
    read: (
      <>
        He asked for the final logo kit — <strong>vector + PNG</strong>.
        I know which folder. Want me to draft the reply with the Drive
        link?
      </>
    ),
    actions: [
      { key: "send-draft", label: "Send draft", kind: "primary", dir: "right" },
      { key: "not-now", label: "Not now", kind: "ghost", dir: "left" },
    ],
  },
];

// ── Stories ───────────────────────────────────────────────────────────────────

export const TodayDeck: Story = {
  name: "Today deck (reply / pay / decide)",
  render: () => (
    <SkimStory
      cards={todayCards}
      clearedTitle="You're clear."
      clearedSub="That's everything that needed you today. Nice work."
    />
  ),
};

export const InboxTriage: Story = {
  name: "Inbox triage (Keep / Archive)",
  render: () => (
    <SkimStory
      cards={inboxCards}
      clearedTitle="Triaged."
      clearedSub="The long tail is sorted. Kept a few, filed the rest."
    />
  ),
};

export const SingleCard: Story = {
  name: "Single card",
  render: () => (
    <SkimStory
      cards={singleCard}
      clearedTitle="Done."
      clearedSub="One and only."
    />
  ),
};

export const ImmediatelyCleared: Story = {
  name: "Immediately cleared (empty deck)",
  render: () => (
    <SkimStory
      cards={[]}
      clearedTitle="You're clear."
      clearedSub="Nothing needs you right now."
    />
  ),
};

import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, fn, userEvent, within } from "storybook/test";

import {
  Banner,
  Button,
  Chip,
  ConversationRow,
  EmptyState,
  Heading,
  Highlight,
  KbdHint,
  Line,
  LineMeta,
  ListPane,
  Overlay,
  PaneHeader,
  ReaderPane,
  SearchField,
  Segmented,
  Shell,
  Spacer,
  UndoProvider,
  type HighlightSegment,
} from "~/lib/ui";

// Fictional mail. Segments are what `search.query` returns for "invoice".
const miss = (text: string): HighlightSegment => ({ text, hit: false });
const hit = (text: string): HighlightSegment => ({ text, hit: true });

const HITS = [
  {
    id: "1",
    from: "Cloudhost Billing",
    service: true,
    time: "Mon",
    title: [miss("Your "), hit("invoice"), miss(" CH-20418 for €148.00")],
    snippet: [miss("Your "), hit("invoice"), miss(" is due on 16 October.")],
    unread: false,
    attachment: true,
  },
  {
    id: "2",
    from: "Ana Lopes",
    service: false,
    time: "Fri",
    title: [miss("Re: October "), hit("invoice"), miss(" for the studio")],
    snippet: [miss("…attached is the corrected "), hit("invoice"), miss(", please confirm.")],
    unread: true,
    count: 3,
  },
  {
    id: "3",
    from: "The Weekly Ledger",
    service: true,
    time: "12 Oct",
    title: [miss("Five tax dates before year end")],
    snippet: [miss("…send your "), hit("invoices"), miss(" before the 31st.")],
    unread: true,
  },
] as const;

const FILTERS = [{ id: "from", label: "From ana", onRemove: fn() }];

function Overlaid({ indexing = false }: { indexing?: boolean }) {
  const [active, setActive] = useState(0);
  return (
    <Overlay open onClose={fn()} label="Search">
      <SearchField
        variant="overlay"
        label="Search conversations"
        placeholder="Search conversations"
        value="invoice from:ana"
        onValueChange={fn()}
        onClear={fn()}
        keys="escape"
        role="combobox"
        aria-expanded="true"
        aria-controls="demo-results"
        aria-activedescendant={`demo-option-${String(active)}`}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown") {
            setActive((index) => (index + 1) % HITS.length);
          } else if (event.key === "ArrowUp") {
            setActive((index) => (index + HITS.length - 1) % HITS.length);
          }
        }}
        chips={FILTERS}
      />
      {indexing ? (
        <div className="px-2 pt-2">
          <Banner tone="neutral" icon="clock" role="status" title="Still indexing older mail">
            Older conversations may be missing from these results for now.
          </Banner>
        </div>
      ) : null}
      <div
        id="demo-results"
        role="listbox"
        aria-label="Results"
        className="flex flex-col gap-0-5 p-1-5"
      >
        {HITS.map((entry, index) => (
          <ConversationRow
            key={entry.id}
            id={`demo-option-${String(index)}`}
            option
            selected={index === active}
            from={entry.from}
            avatar={{ kind: entry.service ? "service" : "person" }}
            time={entry.time}
            subject={<Highlight segments={entry.title} />}
            snippet={<Highlight segments={entry.snippet} />}
            unread={entry.unread}
            count={"count" in entry ? entry.count : undefined}
            attachment={"attachment" in entry}
            onSelect={fn()}
          />
        ))}
      </div>
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-4 py-2">
        <KbdHint keys="arrowup" />
        <KbdHint keys="arrowdown" />
        <KbdHint keys="enter" />
        <Button variant="ghost" keys="mod+enter">
          All results
        </Button>
        <KbdHint keys="escape" />
      </div>
    </Overlay>
  );
}

function Results({ sort = "newest" }: { sort?: "newest" | "relevance" }) {
  const [order, setOrder] = useState(sort);
  return (
    <Shell>
      <ListPane label="Search results">
        <PaneHeader>
          <Heading level={1} size="pane">
            Search
          </Heading>
          <Spacer />
          <Segmented
            label="Order"
            value={order}
            onChange={setOrder}
            options={[
              { value: "newest", label: "Newest" },
              { value: "relevance", label: "Most relevant" },
            ]}
          />
        </PaneHeader>
        <div className="flex flex-col gap-2 px-3 pt-2">
          <SearchField
            label="Search conversations"
            value="invoice from:ana"
            onValueChange={fn()}
            onClear={fn()}
            keys="/"
            chips={FILTERS}
          >
            <div className="flex flex-wrap gap-2" role="group" aria-label="Quick filters">
              <Chip pressed={false}>Has attachment</Chip>
              <Chip pressed>Unread</Chip>
            </div>
          </SearchField>
        </div>
        <div className="flex flex-col gap-0-5 p-1-5">
          {HITS.map((entry, index) => (
            <ConversationRow
              key={entry.id}
              from={entry.from}
              avatar={{ kind: entry.service ? "service" : "person" }}
              time={entry.time}
              subject={<Highlight segments={entry.title} />}
              snippet={<Highlight segments={entry.snippet} />}
              unread={entry.unread}
              selected={index === 1}
              onSelect={fn()}
            />
          ))}
          <div className="flex justify-center p-2">
            <Button variant="ghost">More results</Button>
          </div>
        </div>
      </ListPane>
      <ReaderPane label="Conversation">
        <EmptyState title="Pick a result" visor="idle" visorSize="lg">
          Choose one from the list, or use the keyboard:
        </EmptyState>
      </ReaderPane>
    </Shell>
  );
}

function Empty() {
  return (
    <Shell>
      <ListPane label="Search results">
        <PaneHeader>
          <Heading level={1} size="pane">
            Search
          </Heading>
        </PaneHeader>
        <div className="flex flex-col gap-2 px-3 pt-2">
          <SearchField
            label="Search conversations"
            placeholder="Search conversations"
            value=""
            onValueChange={fn()}
            keys="/"
          />
        </div>
        <section className="flex flex-col gap-1 px-3 pt-3" aria-labelledby="demo-recent">
          <Heading level={2} size="section" id="demo-recent">
            Recent searches
          </Heading>
          <Button variant="ghost" icon="clock" grow>
            lease
          </Button>
          <Button variant="ghost" icon="clock" grow>
            from:ana has:attachment
          </Button>
        </section>
        <section className="flex flex-col gap-1 px-3 py-3" aria-labelledby="demo-tips">
          <Heading level={2} size="section" id="demo-tips">
            Narrow it down
          </Heading>
          <Line trailing={<LineMeta>from:ana</LineMeta>}>From a person</Line>
          <Line trailing={<LineMeta>has:attachment</LineMeta>}>With a file attached</Line>
          <Line trailing={<LineMeta>is:unread</LineMeta>}>Only what you have not read</Line>
        </section>
      </ListPane>
    </Shell>
  );
}

function NoResults() {
  return (
    <Shell>
      <ListPane label="Search results">
        <PaneHeader>
          <Heading level={1} size="pane">
            Search
          </Heading>
        </PaneHeader>
        <div className="flex flex-col gap-2 px-3 pt-2">
          <SearchField
            label="Search conversations"
            value="zebra from:ana"
            onValueChange={fn()}
            onClear={fn()}
            keys="/"
            chips={FILTERS}
          />
        </div>
        <EmptyState title="No conversations match “zebra from:ana”" visor="unsure" visorSize="lg">
          Try removing a filter above, or fewer words.
        </EmptyState>
      </ListPane>
    </Shell>
  );
}

const meta = {
  title: "Patterns/Search",
  parameters: { layout: "fullscreen" },
  decorators: [
    (Story) => (
      <UndoProvider>
        <Story />
      </UndoProvider>
    ),
  ],
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

/** `/`: the overlay with instant rows, the matched words marked, and the filters as chips. */
export const OverlayPanel: Story = {
  render: () => <Overlaid />,
  play: async ({ canvasElement }) => {
    const body = within(canvasElement.ownerDocument.body);
    const field = await body.findByRole("combobox", { name: "Search conversations" });
    await userEvent.type(field, "{ArrowDown}");
    await expect(body.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
  },
};

/** Older mail still being indexed. */
export const Indexing: Story = { render: () => <Overlaid indexing /> };

export const OverlayDark: Story = { render: () => <Overlaid />, globals: { theme: "dark" } };

/** The results page: newest first, a Most relevant toggle, quick filters, the reader beside. */
export const ResultsPage: Story = { render: () => <Results /> };

export const ResultsDark: Story = { render: () => <Results />, globals: { theme: "dark" } };

/** An empty query: the device's recent searches and the operators. */
export const RecentAndTips: Story = { render: () => <Empty /> };

export const NothingMatches: Story = { render: () => <NoResults /> };

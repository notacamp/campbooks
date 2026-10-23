import { fireEvent, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import {
  ChatAbout,
  ChatField,
  ChatMessage,
  ChatNotice,
  ChatPanel,
  ChatProposal,
  ChatRecent,
  ChatSteps,
  ChatSuggestions,
  ChatText,
  ChatWorking,
} from "./Chat";

describe("ChatText", () => {
  it("shows paragraphs and lists as plain text, never markup or links", () => {
    const { container } = renderUi(
      <ChatText
        text={
          "It is under Pay because:\n- <b>Pay</b> by the 1st\n- see https://example.com\n\nThanks."
        }
      />,
    );

    expect(container.querySelectorAll("p")).toHaveLength(2);
    const items = container.querySelectorAll("li");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("<b>Pay</b> by the 1st");
    expect(container.querySelector("b")).toBeNull();
    expect(container.querySelector("a")).toBeNull();
    expect(screen.getByText("see https://example.com")).toBeInTheDocument();
  });
});

describe("ChatPanel", () => {
  it("is a labelled region beside the page, closed by Esc from inside", () => {
    const onClose = vi.fn();
    renderUi(
      <ChatPanel
        modal={false}
        title="Scout"
        closeLabel="Close"
        onClose={onClose}
        about={<ChatAbout label="About">Marta Silva · Rent for November</ChatAbout>}
        footer={<p>footer</p>}
      >
        <ChatMessage from="member" label="You asked" text="Why is this here?" />
      </ChatPanel>,
    );
    const region = screen.getByRole("region", { name: "Scout" });

    expect(within(region).getByText("Marta Silva · Rent for November")).toBeInTheDocument();
    expect(within(region).getByRole("article", { name: "You asked" })).toBeInTheDocument();
    fireEvent.keyDown(within(region).getByRole("button", { name: /Close/ }), { key: "Escape" });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("is a dialog over the screen on a phone, taking the focus", () => {
    renderUi(
      <ChatPanel
        modal
        title="Scout"
        closeLabel="Close"
        onClose={vi.fn()}
        footer={
          <ChatField
            label="Ask about this"
            value=""
            onChange={vi.fn()}
            onAsk={vi.fn()}
            askLabel="Ask"
          />
        }
      >
        {null}
      </ChatPanel>,
    );
    const dialog = screen.getByRole("dialog", { name: "Scout" });

    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByRole("textbox", { name: "Ask about this" })).toHaveFocus();
  });

  it("announces a finished answer politely", () => {
    const { container } = renderUi(
      <ChatPanel
        modal={false}
        title="Scout"
        closeLabel="Close"
        onClose={vi.fn()}
        footer={null}
        announcement="It is a bill."
      >
        {null}
      </ChatPanel>,
    );

    expect(container.querySelector('[aria-live="polite"]')).toHaveTextContent("It is a bill.");
  });
});

describe("ChatPanel over a dialog", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("stands at the page's edge on a wide screen, takes the focus, and the dialog moves aside", () => {
    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: query.includes("64rem"),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    const onClose = vi.fn();
    const { unmount } = renderUi(
      <ChatPanel
        over
        title="Scout"
        closeLabel="Close"
        onClose={onClose}
        footer={
          <ChatField
            label="Ask about this"
            value=""
            onChange={vi.fn()}
            onAsk={vi.fn()}
            askLabel="Ask"
          />
        }
      >
        {null}
      </ChatPanel>,
    );
    const panel = screen.getByRole("dialog", { name: "Scout" });

    expect(panel).toHaveAttribute("aria-modal", "true");
    expect(screen.getByRole("textbox", { name: "Ask about this" })).toHaveFocus();
    expect(document.documentElement).toHaveAttribute("data-chat-beside");
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Ask about this" }), {
      key: "Escape",
    });
    expect(onClose).toHaveBeenCalledOnce();
    unmount();
    expect(document.documentElement).not.toHaveAttribute("data-chat-beside");
  });
});

describe("ChatRecent", () => {
  const items = [
    { id: "c-2", text: "What am I waiting on?", time: "15:02", dateTime: "2026-10-13T15:02:00Z" },
    { id: "c-1", text: "Why is this here?", time: "Yesterday", dateTime: "2026-10-12T09:40:00Z" },
  ];

  it("lists the chats by their first question and time, and opens one", () => {
    const onPick = vi.fn();
    const onOpen = vi.fn();
    renderUi(
      <ChatRecent
        label="Recent"
        items={items}
        empty="No chats yet"
        loading="Loading…"
        current="c-2"
        onOpen={onOpen}
        onPick={onPick}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Recent" }));
    expect(onOpen).toHaveBeenCalledOnce();
    const menu = screen.getByRole("dialog", { name: "Recent" });
    const [first, second] = within(menu).getAllByRole("button");
    expect(first).toHaveTextContent("What am I waiting on?15:02");
    expect(first).toHaveAttribute("aria-current", "true");
    expect(first).toHaveFocus();
    expect(within(menu).getByText("Yesterday")).toHaveAttribute("datetime", "2026-10-12T09:40:00Z");

    fireEvent.keyDown(first as HTMLElement, { key: "ArrowDown" });
    expect(second).toHaveFocus();
    fireEvent.click(second as HTMLElement);
    expect(onPick).toHaveBeenCalledWith("c-1");
    expect(screen.queryByText("Why is this here?")).not.toBeInTheDocument();
  });

  it.each([
    { items: undefined, failed: undefined, says: "Loading…" },
    { items: [], failed: undefined, says: "No chats yet" },
    {
      items: undefined,
      failed: "Your chats couldn’t be listed.",
      says: "Your chats couldn’t be listed.",
    },
  ])("says $says", ({ items: listed, failed, says }) => {
    renderUi(
      <ChatRecent
        label="Recent"
        items={listed}
        empty="No chats yet"
        loading="Loading…"
        failed={failed}
        onPick={vi.fn()}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Recent" }));

    const menu = screen.getByRole("dialog", { name: "Recent" });
    expect(menu).toHaveTextContent(says);
    expect(within(menu).queryAllByRole("button")).toHaveLength(0);
  });
});

describe("ChatField", () => {
  it("asks on Enter and keeps Shift+Enter for a new line", () => {
    const onAsk = vi.fn();
    renderUi(
      <ChatField
        label="Ask about this"
        value="Why?"
        onChange={vi.fn()}
        onAsk={onAsk}
        askLabel="Ask"
        note="Scout looks things up and explains."
      />,
    );
    const box = screen.getByRole("textbox", { name: "Ask about this" });

    fireEvent.keyDown(box, { key: "Enter", shiftKey: true });
    expect(onAsk).not.toHaveBeenCalled();
    fireEvent.keyDown(box, { key: "Enter" });
    expect(onAsk).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: /Ask/ })).toHaveAttribute(
      "aria-keyshortcuts",
      "Enter",
    );
    expect(box).toHaveAccessibleDescription("Scout looks things up and explains.");
  });

  it("stays usable while Scout answers, and Ask waits", () => {
    const onAsk = vi.fn();
    renderUi(
      <ChatField
        label="Ask about this"
        value="Why?"
        onChange={vi.fn()}
        onAsk={onAsk}
        askLabel="Ask"
        busy
      />,
    );
    const box = screen.getByRole("textbox", { name: "Ask about this" });

    fireEvent.keyDown(box, { key: "Enter" });
    expect(onAsk).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: /Ask/ })).toBeDisabled();
    expect(box).toBeEnabled();
  });

  it("asks nothing empty", () => {
    const onAsk = vi.fn();
    renderUi(
      <ChatField
        label="Ask about this"
        value="  "
        onChange={vi.fn()}
        onAsk={onAsk}
        askLabel="Ask"
      />,
    );
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });

    expect(onAsk).not.toHaveBeenCalled();
  });
});

describe("ChatSuggestions, ChatSteps, ChatWorking and ChatNotice", () => {
  it("asks a tapped question, lists what was looked at and opens what it found", () => {
    const onPick = vi.fn();
    const onOpen = vi.fn();
    renderUi(
      <>
        <ChatSuggestions
          label="Questions to start from"
          intro="Ask me about Marta's email, Rent for November."
          questions={["Why is this here?", "What else has Marta sent?"]}
          onPick={onPick}
        />
        <ChatSteps
          summary="What I looked at"
          steps={[
            { text: "Read what happened to this email" },
            {
              text: "Looked for conversations: 1 found",
              records: [{ id: "c-1", label: "Marta Silva · Rent", onOpen }],
            },
          ]}
        />
        <ChatWorking
          lines={["Reading what happened to this email…", "Looking for conversations…"]}
        />
        <ChatNotice action={<button type="button">Try again</button>}>
          Scout couldn&apos;t answer that.
        </ChatNotice>
      </>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Why is this here?" }));
    expect(onPick).toHaveBeenCalledWith("Why is this here?");
    fireEvent.click(screen.getByText("What I looked at"));
    fireEvent.click(screen.getByRole("button", { name: "Marta Silva · Rent" }));
    expect(onOpen).toHaveBeenCalledOnce();
    expect(screen.getByText("Looking for conversations…")).toBeInTheDocument();
    expect(screen.getByText("Scout couldn't answer that.")).toBeInTheDocument();
  });

  it("shows no steps when there are none", () => {
    const { container } = renderUi(<ChatSteps summary="What I looked at" steps={[]} />);

    expect(container).toBeEmptyDOMElement();
  });
});

describe("ChatProposal", () => {
  it("names the change and what it is about, quotes Scout's words, and offers the choice", () => {
    const confirm = vi.fn();
    renderUi(
      <ChatProposal
        label="Scout proposes"
        title="Add the label"
        about="Marta Silva · Rent for November"
        quote="<b>Accountant</b>"
        quoteLabel="Scout’s words"
        state="pending"
        actions={
          <button type="button" onClick={confirm}>
            Confirm
          </button>
        }
      />,
    );

    const card = screen.getByRole("region", { name: "Add the label" });
    expect(within(card).getByText("Marta Silva · Rent for November")).toBeVisible();
    // Plain text, quoted: never markup.
    const quote = within(card).getByText("<b>Accountant</b>");
    expect(quote.tagName).toBe("Q");
    expect(card.querySelector("b")).toBeNull();
    // Nothing to say yet: the status line is empty.
    expect(within(card).queryByRole("status")).not.toBeInTheDocument();
    fireEvent.click(within(card).getByRole("button", { name: "Confirm" }));
    expect(confirm).toHaveBeenCalledOnce();
  });

  it("says how a settled card ended, without buttons", () => {
    renderUi(
      <ChatProposal
        label="Scout proposed"
        title="Archive"
        about="Marta Silva · Rent for November"
        state="failed"
        status="This couldn’t be done."
      />,
    );

    const card = screen.getByRole("region", { name: "Archive" });
    expect(within(card).getByRole("status")).toHaveTextContent("This couldn’t be done.");
    expect(within(card).queryByRole("button")).toBeNull();
  });
});

import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../test/render";

import {
  AccountButton,
  ListPane,
  PaneHeader,
  ReaderBody,
  ReaderFooter,
  ReaderPane,
  SearchButton,
  Shell,
  Sidebar,
  SidebarFooter,
  PhoneHeader,
  Spacer,
  StatusLine,
  WorkspaceMark,
  WorkspaceSwitcher,
} from "./Shell";

describe("Shell", () => {
  it("lays out named landmarks", () => {
    const onSwitch = vi.fn();
    const onSearch = vi.fn();
    renderUi(
      <Shell>
        <Sidebar>
          <WorkspaceSwitcher mark="DP" name="Duarte & Pessoa" kind="Business" onClick={onSwitch} />
          <SearchButton onClick={onSearch} />
          <SidebarFooter>
            <StatusLine visor="reading" title="Reading 3 new messages">
              48 sorted since Friday
            </StatusLine>
          </SidebarFooter>
        </Sidebar>
        <ListPane label="Conversations">
          <PaneHeader>
            <h1>Primary</h1>
            <Spacer />
          </PaneHeader>
        </ListPane>
        <ReaderPane label="Conversation">
          <ReaderBody>
            <p>Body</p>
          </ReaderBody>
        </ReaderPane>
      </Shell>,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Switch workspace (current: Duarte & Pessoa)" }),
    );
    fireEvent.click(screen.getByRole("button", { name: /Search or ask/ }));

    expect(onSwitch).toHaveBeenCalledOnce();
    expect(onSearch).toHaveBeenCalledOnce();
    expect(screen.getByRole("navigation", { name: "Main" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Conversations" })).toBeInTheDocument();
    expect(screen.getByRole("main", { name: "Conversation" })).toHaveTextContent("Body");
    expect(screen.getByText("Reading 3 new messages")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Search or ask/ })).toHaveAttribute(
      "aria-keyshortcuts",
      "Control+K",
    );
  });

  it("pins a reader's footer after its body: the composer, or else the prompt", () => {
    const { rerender } = renderUi(
      <ReaderPane label="Conversation">
        <ReaderBody>
          <p>Thread</p>
        </ReaderBody>
        <ReaderFooter
          slot={null}
          prompt={<button type="button">Reply to Marta</button>}
          phoneBar={<button type="button">Done</button>}
        />
      </ReaderPane>,
    );
    const reader = screen.getByRole("main", { name: "Conversation" });
    const prompt = screen.getByRole("button", { name: "Reply to Marta" });

    // After the thread, never inside it.
    expect(reader.lastElementChild).toContainElement(prompt);
    expect(screen.getByText("Thread").compareDocumentPosition(prompt)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
    expect(screen.getByRole("button", { name: "Done" })).toBeInTheDocument();

    rerender(
      <ReaderPane label="Conversation">
        <ReaderBody>
          <p>Thread</p>
        </ReaderBody>
        <ReaderFooter slot={<form aria-label="Reply to Marta Silva" />} />
      </ReaderPane>,
    );
    expect(screen.getByRole("form", { name: "Reply to Marta Silva" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Reply to Marta" })).toBeNull();
  });

  it("shows only the mark when compact", () => {
    renderUi(
      <WorkspaceSwitcher compact mark="AD" name="Ana Duarte" kind="Personal" onClick={vi.fn()} />,
      { locale: "pt-PT" },
    );
    const button = screen.getByRole("button", {
      name: "Mudar de espaço de trabalho (atual: Ana Duarte)",
    });

    expect(button).toHaveTextContent("AD");
    expect(button).toHaveAttribute("aria-keyshortcuts", "Control+Shift+W");
  });
});

describe("PhoneHeader, AccountButton and WorkspaceMark", () => {
  it("shows who is signed in, in full or as an avatar", () => {
    renderUi(
      <>
        <PhoneHeader>
          <WorkspaceMark mark="HO" accent="personal" size="lg" />
        </PhoneHeader>
        <AccountButton name="Ana Duarte" email="ana@example.com" />
        <AccountButton compact label="Account" name="Ana Duarte" />
        <AccountButton name="ana@example.com" email="ana@example.com" />
      </>,
    );

    // Hidden from the medium breakpoint up (jsdom's window is desktop-wide).
    expect(screen.getByRole("banner", { hidden: true })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Ana Duarte/u })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Account" })).toBeInTheDocument();
    expect(screen.getAllByText("ana@example.com")).toHaveLength(2);
    expect(screen.getByText("HO")).toHaveAttribute("data-accent", "personal");
  });
});

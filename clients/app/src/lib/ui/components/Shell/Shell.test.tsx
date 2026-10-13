import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import {
  AccountButton,
  ListPane,
  PaneHeader,
  ReaderBody,
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

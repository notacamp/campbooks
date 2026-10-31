import { fireEvent, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { DocCard, DocImage, DocToggle, MentionChip, RichDocument } from "./RichDocument";
import stylesheet from "./RichDocument.module.css?raw";

describe("RichDocument", () => {
  it("a mention is a link when it can be opened and plain words when it cannot", () => {
    const onOpen = vi.fn((event: { preventDefault: () => void }) => {
      event.preventDefault();
    });
    renderUi(
      <RichDocument>
        <p>
          <MentionChip kind="thread" label="Kickoff agenda" href="/t/1" onOpen={onOpen} />{" "}
          <MentionChip kind="thread" label="A thread you can't open" state="closed" />
        </p>
      </RichDocument>,
    );
    fireEvent.click(screen.getByRole("link", { name: "Kickoff agenda" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("link", { name: /can't open/u })).toBeNull();
    expect(screen.getByText("A thread you can't open")).toBeInTheDocument();
  });

  it("a card downloads a file and hides what a viewer cannot open", () => {
    renderUi(
      <RichDocument>
        <DocCard icon="attachment" title="brand-guide.pdf" href="/f" download="brand-guide.pdf" />
        <DocCard icon="mail" title="Something you can't open" href="/t" state="closed" />
      </RichDocument>,
    );
    const link = screen.getByRole("link", { name: /brand-guide\.pdf/u });
    expect(link).toHaveAttribute("download", "brand-guide.pdf");
    expect(screen.queryByRole("link", { name: /can't open/u })).toBeNull();
  });

  it("a toggle reports its state and asks to change it", () => {
    const onToggle = vi.fn();
    renderUi(
      <DocToggle open={false} label="Show what is inside" onToggle={onToggle}>
        <p>Summary</p>
      </DocToggle>,
    );
    const arrow = screen.getByRole("button", { name: "Show what is inside" });
    expect(arrow).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(arrow);
    expect(onToggle).toHaveBeenCalledTimes(1);
  });

  it("an image the viewer cannot open is not loaded", () => {
    renderUi(<DocImage src="/x.png" alt="Diagram" state="closed" />);
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByText("Diagram")).toBeInTheDocument();
  });

  it("names the editor's own classes only through :global(), so the module keeps them", () => {
    // In a CSS module a bare `.tiptap` is renamed and never matches the editor.
    const bare = stylesheet
      .replaceAll(/:global\([^)]*\)/gu, "")
      .match(/\.(?:tiptap|ProseMirror[\w-]*|react-renderer|node-[\w-]+)\b/gu);
    expect(bare).toBeNull();
  });
});

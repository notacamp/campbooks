import { fireEvent, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { renderUi } from "../../../../../test/render";

import { AttachmentList, focusNextAttachment, ImagePreview } from "./Attachments";

const FILES = [
  { id: "a1", name: "harbour.jpg", size: 284_211, kind: "image" as const },
  { id: "a2", name: "invoice.pdf", size: 18_406, kind: "pdf" as const },
  { id: "a3", name: "statement.html", size: 920, kind: "file" as const },
];

/** Row `index` of the list. */
function row(index: number): HTMLElement {
  const element = screen.getAllByRole("listitem")[index];
  if (element === undefined) {
    throw new Error(`no row ${String(index)}`);
  }
  return element;
}

describe("AttachmentList", () => {
  it("lists name, size and actions; only images and PDFs have a preview", () => {
    const onPreview = vi.fn();
    const onDownload = vi.fn();
    renderUi(<AttachmentList attachments={FILES} onPreview={onPreview} onDownload={onDownload} />);
    const items = screen.getAllByRole("listitem");

    expect(items).toHaveLength(3);
    expect(items[0]).toHaveTextContent("harbour.jpg");
    expect(items[0]).toHaveTextContent("277.5 kB");
    expect(within(row(0)).getAllByRole("button")).toHaveLength(3);
    // HTML is download-only: its name downloads, and there is no preview.
    const html = within(row(2));
    expect(html.queryByRole("button", { name: /^Preview/u })).toBeNull();
    fireEvent.click(html.getByRole("button", { name: "statement.html" }));
    expect(onDownload).toHaveBeenLastCalledWith("a3");
    fireEvent.click(html.getByRole("button", { name: "Download statement.html" }));
    expect(onDownload).toHaveBeenCalledTimes(2);
    // An image's name opens its preview.
    fireEvent.click(screen.getByRole("button", { name: "harbour.jpg" }));
    expect(onPreview).toHaveBeenLastCalledWith("a1");
    fireEvent.click(screen.getByRole("button", { name: "Preview invoice.pdf" }));
    expect(onPreview).toHaveBeenLastCalledWith("a2");
  });

  it("shows a failure and the attachment being fetched", () => {
    renderUi(
      <AttachmentList
        attachments={FILES}
        onPreview={vi.fn()}
        onDownload={vi.fn()}
        busy="a2"
        error="Could not open invoice.pdf."
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("Could not open invoice.pdf.");
    expect(screen.getAllByRole("listitem")[1]).toHaveAttribute("aria-busy", "true");
  });

  it("moves focus through the attachments, wrapping around", () => {
    renderUi(<AttachmentList attachments={FILES} onPreview={vi.fn()} onDownload={vi.fn()} />);
    const names = screen
      .getAllByRole("listitem")
      .map((item) => item.querySelector<HTMLButtonElement>("[data-attachment-open]"));

    expect(focusNextAttachment()).toBe(true);
    expect(document.activeElement).toBe(names[0]);
    focusNextAttachment();
    focusNextAttachment();
    expect(document.activeElement).toBe(names[2]);
    focusNextAttachment();
    expect(document.activeElement).toBe(names[0]);
  });

  it("has nothing to move to without attachments", () => {
    renderUi(<AttachmentList attachments={[]} onPreview={vi.fn()} onDownload={vi.fn()} />);

    expect(screen.queryByRole("list")).toBeNull();
    expect(focusNextAttachment()).toBe(false);
  });
});

describe("ImagePreview", () => {
  it("shows the picture as an image in a dialog that Escape closes", () => {
    const onClose = vi.fn();
    renderUi(
      <ImagePreview
        src="blob:http://localhost/1"
        name="harbour.jpg"
        onClose={onClose}
        onDownload={vi.fn()}
      />,
    );
    const dialog = screen.getByRole("dialog", { name: "Preview of harbour.jpg" });

    expect(within(dialog).getByRole("img", { name: "harbour.jpg" })).toHaveAttribute(
      "src",
      "blob:http://localhost/1",
    );
    expect(dialog.querySelector("iframe, object, embed")).toBeNull();
    fireEvent.keyDown(dialog, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });
});

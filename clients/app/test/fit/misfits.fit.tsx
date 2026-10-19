// The fit check's own test: it reports our copy cut off, off screen or as a
// placeholder too wide for its field, and leaves alone what fits and demo
// content (unmarked) that truncates on purpose.
import { afterEach, describe, expect, it } from "vitest";
import { page } from "vitest/browser";

import { FIT_MARK } from "../../.storybook/longest";
import { misfits } from "./misfits";

const copy = (text: string) => `${FIT_MARK}${text}${FIT_MARK}`;

function mount(html: string): void {
  const host = document.createElement("div");
  host.dataset["fitFixture"] = "";
  host.innerHTML = html;
  document.body.append(host);
}

const ELLIPSIS = "width: 80px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap";

describe("misfits", () => {
  afterEach(() => {
    for (const host of document.querySelectorAll("[data-fit-fixture]")) {
      host.remove();
    }
  });

  it("reports a label cut short by an ellipsis", async () => {
    await page.viewport(800, 600);
    mount(
      `<button><span style="display: block; ${ELLIPSIS}">${copy("Bandeja de entrada")}</span></button>`,
    );
    expect(misfits()).toEqual([
      expect.objectContaining({ text: "Bandeja de entrada", problem: "cut off" }),
    ]);
  });

  it("reports copy clipped by an ancestor", async () => {
    await page.viewport(800, 600);
    mount(
      `<div style="width: 60px; overflow: hidden"><span style="white-space: nowrap">${copy("Synchronisation en cours")}</span></div>`,
    );
    expect(misfits()).toEqual([expect.objectContaining({ problem: "cut off" })]);
  });

  it("reports a placeholder wider than its field", async () => {
    await page.viewport(800, 600);
    mount(
      `<input style="width: 60px" placeholder="${copy("Rechercher dans la boîte de réception")}">`,
    );
    expect(misfits()).toEqual([expect.objectContaining({ problem: "placeholder cut off" })]);
  });

  it("reports a page that scrolls sideways", async () => {
    await page.viewport(375, 600);
    mount(
      `<div style="display: flex; white-space: nowrap"><span>${copy("Aplicação do espaço de trabalho ".repeat(4))}</span></div>`,
    );
    expect(misfits().map((misfit) => misfit.problem)).toContain("page scrolls sideways");
  });

  it("leaves alone copy that fits and demo content that truncates", async () => {
    await page.viewport(800, 600);
    mount(`
      <span style="display: block; ${ELLIPSIS}">${copy("Inbox")}</span>
      <span style="display: block; ${ELLIPSIS}">Maria do Carmo Sant’Ana Albuquerque</span>
      <div style="width: 80px">${copy("Aplicação do espaço de trabalho")}</div>
      <div style="width: 0; height: 0; overflow: hidden">${copy("Visually hidden")}</div>
    `);
    expect(misfits()).toEqual([]);
  });
});

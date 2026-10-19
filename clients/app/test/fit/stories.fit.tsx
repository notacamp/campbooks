// Copy fits in every language (`pnpm test:fit`): every story, rendered in the
// "longest" pseudo-locale (each string in its longest translation, see
// `.storybook/longest.ts`), at phone and desktop widths, must show all of our
// copy: nothing cut off, nothing off screen, no sideways scroll.
//
// Stories render without their play functions: those drive the English UI by
// its English names. The states they reach (open menus, dialogs) are covered
// where a story renders them open by itself.
import {
  composeStory,
  setProjectAnnotations,
  type Meta,
  type StoryObj,
} from "@storybook/react-vite";
import { createRoot } from "react-dom/client";
import { beforeAll, describe, expect, it } from "vitest";
import { page } from "vitest/browser";

import { LONGEST } from "../../.storybook/longest";
import preview from "../../.storybook/preview";
import { formatMisfit, misfits, settle } from "./misfits";

setProjectAnnotations([preview, { initialGlobals: { locale: LONGEST, theme: "light" } }]);

const VIEWPORTS = [
  { name: "phone", width: 375, height: 812 },
  { name: "desktop", width: 1280, height: 800 },
] as const;

type Annotations = Record<string, unknown>;

const modules = import.meta.glob<Annotations>(
  ["../../src/**/*.stories.tsx", "../../stories/**/*.stories.tsx"],
  { eager: true },
);

function withoutPlay(annotations: Annotations): Annotations {
  const copy = { ...annotations };
  delete copy["play"];
  return copy;
}

const isStoryName = (name: string, value: unknown) =>
  /^[A-Z]/u.test(name) && (typeof value === "object" || typeof value === "function");

/** Every story of every stories file, as `[file, story name, composed story]`. */
function allStories() {
  return Object.entries(modules).flatMap(([file, module]) => {
    const { default: meta, ...exports } = module;
    const component = withoutPlay((meta ?? {}) as Annotations) as Meta;
    return Object.entries(exports)
      .filter(([name, story]) => isStoryName(name, story))
      .map(
        ([name, story]) =>
          [
            file.replace(/^(\.\.\/)+/u, ""),
            name,
            composeStory(withoutPlay(story as Annotations) as StoryObj, component, undefined, name),
          ] as const,
      );
  });
}

const stories = allStories();

describe.each(VIEWPORTS)("copy fits at $name width ($width px)", ({ width, height }) => {
  beforeAll(async () => {
    await page.viewport(width, height);
  });

  it.each(stories)("%s › %s", async (_file, _name, Story) => {
    await Story.load();
    const container = document.createElement("div");
    document.body.append(container);
    const root = createRoot(container);
    try {
      root.render(<Story />);
      await settle();
      expect(misfits().map(formatMisfit)).toEqual([]);
    } finally {
      root.unmount();
      container.remove();
    }
  });
});

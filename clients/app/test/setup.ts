import "@testing-library/jest-dom/vitest";

import { cleanup, configure } from "@testing-library/react";
import { afterEach, vi } from "vitest";

// The first render of a module screen loads its i18n namespace lazily, which
// can take longer than the default second on a cold, busy CI runner.
configure({ asyncUtilTimeout: 5000 });

afterEach(() => {
  cleanup();
});

// jsdom does not implement scrolling; the router restores scroll on navigation.
Object.defineProperty(globalThis, "scrollTo", { value: vi.fn(), writable: true });

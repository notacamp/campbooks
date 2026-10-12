import "@testing-library/jest-dom/vitest";

import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
});

// jsdom does not implement scrolling; the router restores scroll on navigation.
Object.defineProperty(globalThis, "scrollTo", { value: vi.fn(), writable: true });

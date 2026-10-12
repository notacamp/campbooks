import type { Page } from "playwright";

/** What the recorder hands to every scenario. */
export interface ScenarioContext {
  page: Page;
}

/** Linger on the current state so the recording shows it. */
export function pause(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

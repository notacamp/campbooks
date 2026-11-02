import type { MouseEvent } from "react";

/** A link that follows `onNavigate` for a plain click, and the browser for the rest. */
export function followIn(onNavigate: ((href: string) => void) | undefined, href: string) {
  return {
    href,
    onClick: (event: MouseEvent<HTMLElement>) => {
      if (onNavigate === undefined || event.metaKey || event.ctrlKey || event.button !== 0) {
        return;
      }
      event.preventDefault();
      onNavigate(href);
    },
  };
}

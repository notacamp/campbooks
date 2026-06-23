import { useEffect } from "react";

/**
 * Reveals elements with the `cb-reveal` class as they scroll into view.
 * SSR-safe (runs only in useEffect). Honors prefers-reduced-motion by
 * showing everything immediately. Stagger via inline `--cb-delay`.
 */
export function useScrollReveal(): void {
  useEffect(() => {
    const els = Array.from(
      document.querySelectorAll<HTMLElement>(".cb-reveal"),
    );
    if (els.length === 0) return;

    // Capture/escape hatch: ?cap reveals everything immediately (for screenshots).
    if (window.location.search.includes("cap")) {
      els.forEach((el) => el.classList.add("cb-in"));
      return;
    }

    const reduce = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    if (reduce || typeof IntersectionObserver === "undefined") {
      els.forEach((el) => el.classList.add("cb-in"));
      return;
    }

    const io = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            entry.target.classList.add("cb-in");
            io.unobserve(entry.target);
          }
        }
      },
      { rootMargin: "0px 0px -8% 0px", threshold: 0.12 },
    );
    els.forEach((el) => io.observe(el));
    return () => io.disconnect();
  }, []);
}

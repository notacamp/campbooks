/**
 * lib/store/ui — Zustand UI state store.
 *
 * Thin: client-only state like which panel is open, which row is selected, or
 * whether the command menu is visible. Never mirrors server data here — that
 * belongs in TanStack Query. Server data in Zustand creates a second cache that
 * goes stale and causes subtle bugs.
 *
 * Usage pattern (demonstrating the Zustand v5 store shape):
 *   const isCommandMenuOpen = useUIStore((s) => s.commandMenuOpen);
 *   const { openCommandMenu } = useUIStore();
 */
import { create } from "zustand";

interface UIState {
  /** True when the ⌘K command menu is open. */
  commandMenuOpen: boolean;
  /** Open the ⌘K command menu. */
  openCommandMenu: () => void;
  /** Close the ⌘K command menu. */
  closeCommandMenu: () => void;
  /** Toggle the ⌘K command menu. */
  toggleCommandMenu: () => void;
}

export const useUIStore = create<UIState>()((set) => ({
  commandMenuOpen: false,
  openCommandMenu: () => set({ commandMenuOpen: true }),
  closeCommandMenu: () => set({ commandMenuOpen: false }),
  toggleCommandMenu: () =>
    set((state) => ({ commandMenuOpen: !state.commandMenuOpen })),
}));

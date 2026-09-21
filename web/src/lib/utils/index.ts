/**
 * lib/utils — reusable, business-agnostic helpers.
 *
 * `cn()` is the shadcn/ui utility that merges class-variance-authority output
 * with Tailwind class de-duplication via tailwind-merge. lib/ui is the only
 * consumer; modules must not import from here directly (they have no reason to
 * touch class strings — lib/ui components take directive props instead).
 */
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** Merge Tailwind utility classes, resolving conflicts in the rightmost wins. */
export const cn = (...inputs: ClassValue[]): string => twMerge(clsx(inputs));

/**
 * Vitest test setup — runs before every test file.
 * Extends Vitest's expect with @testing-library/jest-dom matchers
 * (toBeInTheDocument, toBeDisabled, toHaveClass, etc.).
 */
import "@testing-library/jest-dom/vitest";

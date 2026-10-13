// Allowed: the design system's public surface.
import { thing } from "../../lib/ui";
// Forbidden: reaching into a component.
import { thing as deep } from "../../lib/ui/components/thing";
// Forbidden: a module stylesheet (visual styling lives in lib/ui).
import "./gamma.css";

export const gamma = thing + deep;

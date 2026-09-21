/**
 * ExtensionPoint — renders all enabled extensions registered for a named point.
 *
 * Each extension is lazy-loaded, mounted behind its own Suspense + error
 * boundary. A throwing extension blanks itself silently; the page continues.
 * A disabled extension (flag not resolved) is not loaded at all.
 */
import {
  lazy,
  Suspense,
  Component,
  type ReactNode,
  type ErrorInfo,
  type ComponentType,
} from "react";
import {
  getExtensions,
  isExtensionEnabled,
  type ExtensionPointName,
  type ExtensionPointContracts,
} from "./registry";

// ── Error boundary ────────────────────────────────────────────────────────────

interface ErrorBoundaryState {
  hasError: boolean;
}

// Must be a class component — React has no hook equivalent for componentDidCatch.
class ExtensionErrorBoundary extends Component<
  { children: ReactNode },
  ErrorBoundaryState
> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(): ErrorBoundaryState {
    return { hasError: true };
  }

  componentDidCatch(_error: Error, _info: ErrorInfo): void {
    // Silently suppress — a broken extension must not crash the page.
    // In production, an observability sink would forward this to error-tracking.
  }

  render(): ReactNode {
    if (this.state.hasError) return null;
    return this.props.children;
  }
}

// ── ExtensionPoint ────────────────────────────────────────────────────────────

type ExtensionPointProps<N extends ExtensionPointName> = {
  name: N;
  props: ExtensionPointContracts[N];
};

/**
 * Renders all enabled, lazy-loaded extensions registered for `name`.
 *
 * Usage (in an owner's page component):
 *   <ExtensionPoint name="now.feed.sections" props={{ date: today }} />
 */
export const ExtensionPoint = <N extends ExtensionPointName>({
  name,
  props: extensionProps,
}: ExtensionPointProps<N>): ReactNode => {
  const extensions = getExtensions(name).filter(isExtensionEnabled);

  return (
    <>
      {extensions.map((ext) => {
        // Cast to a known component type to satisfy TypeScript — the runtime
        // shape is guaranteed by the Extension descriptor's load() contract.
        const LazyExt = lazy(
          ext.load as () => Promise<{
            default: ComponentType<Record<string, unknown>>;
          }>,
        );
        const props = extensionProps as Record<string, unknown>;
        return (
          <ExtensionErrorBoundary key={ext.id}>
            <Suspense fallback={null}>
              <LazyExt {...props} />
            </Suspense>
          </ExtensionErrorBoundary>
        );
      })}
    </>
  );
};

/**
 * InboxPage integration test — mocked fetch, no live server.
 *
 * Tests the archive → undo flow with optimistic updates:
 *   1. Renders list of rows from mocked GET /api/app/people
 *   2. Clicking archive removes the row optimistically
 *   3. UndoDock appears; clicking Undo calls unarchive and restores the row
 *
 * Selectors: data-testid ONLY (convention in this codebase).
 * data-testid pattern: <module>.<feature>.<element>[.<part>][.<id>]
 */
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { InboxRow } from "~/modules/inbox/types";
import { InboxPage } from "./InboxPage";

// ── Mocks ─────────────────────────────────────────────────────────────────────

// Prevent realtime WebSocket from opening in tests
vi.mock("~/lib/realtime", () => ({
  subscribeUserSync: () => () => undefined,
}));

// Mock sessionStorage (token lookup) — returns a test token
const mockGetItem = vi.fn(() => "test-token");
Object.defineProperty(window, "sessionStorage", {
  value: { getItem: mockGetItem, setItem: vi.fn(), removeItem: vi.fn() },
  writable: true,
});

// ── Fixtures ──────────────────────────────────────────────────────────────────

const makeRow = (overrides: Partial<InboxRow> = {}): InboxRow => ({
  id: 1,
  counterpart_type: "Contact",
  name: "Sofia Andrade",
  subtitle: "Your project proposal",
  avatar_initial: "SA",
  avatar_email: "sofia@example.com",
  needs_you: true,
  verb: "reply",
  stand_line: "Waiting on your reply since Tuesday",
  wait_days: 3,
  unread: true,
  score: 90,
  last_activity_at: new Date(Date.now() - 3_600_000).toISOString(),
  email_message_id: 42,
  feed_item_id: 7,
  standing_kind: "reply",
  ...overrides,
});

const ROW_A = makeRow({ id: 1, name: "Sofia Andrade" });
const ROW_B = makeRow({ id: 2, name: "João Silva", verb: "pay", needs_you: false, unread: false });

const makeCollectionResponse = (rows: InboxRow[]): string =>
  JSON.stringify({
    data: rows,
    meta: { page: 1, per_page: 30, total: rows.length, total_pages: 1 },
  });

const makeActionResponse = (): string => JSON.stringify({ data: null });

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Creates a fresh QueryClient with no caching/retries (for clean test isolation). */
const createTestQueryClient = (): QueryClient =>
  new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  });

/** Wraps the component with QueryClientProvider. */
const renderInbox = (queryClient: QueryClient): ReturnType<typeof render> =>
  render(
    <QueryClientProvider client={queryClient}>
      <InboxPage />
    </QueryClientProvider>,
  );

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("InboxPage", () => {
  let fetchMock: ReturnType<typeof vi.fn>;
  let queryClient: QueryClient;

  beforeEach(() => {
    queryClient = createTestQueryClient();
    fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    queryClient.clear();
  });

  it("renders the list of rows after loading", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => JSON.parse(makeCollectionResponse([ROW_A, ROW_B])),
    });

    renderInbox(queryClient);

    // Initially loading skeletons appear
    expect(screen.getByTestId("inbox.list.loading")).toBeInTheDocument();

    // After fetch resolves, rows appear
    await screen.findByTestId("inbox.list.root");

    expect(screen.getByTestId(`inbox.list.row.${ROW_A.id}`)).toBeInTheDocument();
    expect(screen.getByTestId(`inbox.list.row.${ROW_B.id}`)).toBeInTheDocument();
  });

  it("optimistically removes a row when archived and shows the undo dock", async () => {
    fetchMock
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeCollectionResponse([ROW_A, ROW_B])),
      })
      // Archive POST
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeActionResponse()),
      })
      // Refetch after settle
      .mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeCollectionResponse([ROW_B])),
      });

    renderInbox(queryClient);

    await screen.findByTestId("inbox.list.root");

    // Both rows visible before archive
    expect(screen.getByTestId(`inbox.list.row.${ROW_A.id}`)).toBeInTheDocument();

    // Click archive button on ROW_A
    const archiveBtn = screen.getByTestId(`inbox.list.archive.${ROW_A.id}`);
    fireEvent.click(archiveBtn);

    // Row A should be optimistically removed (async onMutate awaits cancelQueries first)
    await waitFor(() =>
      expect(screen.queryByTestId(`inbox.list.row.${ROW_A.id}`)).not.toBeInTheDocument(),
    );

    // UndoDock should appear
    await screen.findByTestId("inbox.undodock.root");
    expect(screen.getByTestId("inbox.undodock.undo")).toBeInTheDocument();
  });

  it("restores the row when Undo is clicked", async () => {
    fetchMock
      // Initial list
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeCollectionResponse([ROW_A, ROW_B])),
      })
      // Archive POST
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeActionResponse()),
      })
      // Refetch after archive settle (without ROW_A)
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeCollectionResponse([ROW_B])),
      })
      // Unarchive POST
      .mockResolvedValueOnce({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeActionResponse()),
      })
      // Refetch after unarchive settle (ROW_A is back)
      .mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => JSON.parse(makeCollectionResponse([ROW_A, ROW_B])),
      });

    renderInbox(queryClient);

    await screen.findByTestId("inbox.list.root");

    // Archive ROW_A
    fireEvent.click(screen.getByTestId(`inbox.list.archive.${ROW_A.id}`));

    // Wait for undo dock
    await screen.findByTestId("inbox.undodock.root");

    // Click Undo
    fireEvent.click(screen.getByTestId("inbox.undodock.undo"));

    // ROW_A should come back after the refetch
    expect(await screen.findByTestId(`inbox.list.row.${ROW_A.id}`)).toBeInTheDocument();
  });

  it("shows the empty state when there are no rows", async () => {
    fetchMock.mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => JSON.parse(makeCollectionResponse([])),
    });

    renderInbox(queryClient);

    expect(await screen.findByTestId("inbox.list.empty")).toBeInTheDocument();
  });

  it("shows the error state and retry button on fetch failure", async () => {
    fetchMock.mockRejectedValue(new Error("Network error"));

    renderInbox(queryClient);

    await screen.findByTestId("inbox.list.error");

    expect(screen.getByTestId("inbox.list.retry")).toBeInTheDocument();
  });
});

import * as Y from "yjs";

import { fromBase64, toBase64 } from "./bytes";

/** Where a page's body stands with the server. */
export type SaveState =
  /** Everything typed is on the server. */
  | "saved"
  /** A call is on its way, or changes wait to go. */
  | "saving"
  /** The network or the server failed; it keeps trying. */
  | "retrying"
  /** The server refused a change (it is not valid): nothing more is sent. */
  | "rejected";

export interface SyncReply {
  /** What the client lacked, as a Yjs update in base64. */
  update: string;
  /** The server's state vector in base64. */
  state_vector: string;
}

/** One call to `pages.sync`: what the client holds, and what it adds. */
export type SyncCall = (body: { state_vector: string; update?: string }) => Promise<SyncReply>;

export interface PageSyncOptions {
  /** How long typing is gathered before it is sent, in ms. */
  debounceMs?: number | undefined;
  /** How long to wait before each retry, in ms; the last repeats. */
  retryMs?: readonly number[] | undefined;
  /** Whether a failure is the server refusing the change, not a network that is down. */
  refused?: ((error: unknown) => boolean) | undefined;
}

const REMOTE = Symbol("remote");

/** An update with nothing in it: no structs, no deletions. */
const isEmpty = (update: Uint8Array) => update.length <= 2;

/**
 * Keeps one page's Yjs document in step with the server through
 * `pages.sync`, with one call on its way at a time. Local edits are gathered,
 * then sent as what the server's last state vector lacks; the answer carries
 * what others wrote, applied without echoing it back. A failure is retried
 * with a growing wait, unless the server refused the change as invalid.
 */
export class PageSync {
  private readonly doc: Y.Doc;
  private readonly call: SyncCall;
  private readonly debounceMs: number;
  private readonly retryMs: readonly number[];
  private readonly refused: (error: unknown) => boolean;
  private server: Uint8Array;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private running = false;
  private again = false;
  private failures = 0;
  private stopped = false;
  private state: SaveState = "saved";
  private readonly listeners = new Set<(state: SaveState) => void>();
  private readonly onUpdate = (_update: Uint8Array, origin: unknown) => {
    if (origin !== REMOTE && this.state !== "rejected") {
      this.schedule();
    }
  };

  /**
   * @param doc The page's document, already holding what the server sent.
   * @param serverVector The state vector the server answered with.
   */
  constructor(
    doc: Y.Doc,
    serverVector: Uint8Array,
    call: SyncCall,
    options: PageSyncOptions = {},
  ) {
    this.doc = doc;
    this.server = serverVector;
    this.call = call;
    this.debounceMs = options.debounceMs ?? 400;
    this.retryMs = options.retryMs ?? [1000, 2000, 4000, 8000, 15_000, 30_000];
    this.refused = options.refused ?? (() => false);
    doc.on("update", this.onUpdate);
    // Anything typed before this started is not on the server yet.
    if (!isEmpty(Y.encodeStateAsUpdate(doc, serverVector))) {
      this.schedule();
    }
  }

  /** The state now. */
  get current(): SaveState {
    return this.state;
  }

  /** Tell `listener` whenever the state changes; returns how to stop. */
  subscribe(listener: (state: SaveState) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Stop listening to the document and sending. */
  stop(): void {
    this.stopped = true;
    this.doc.off("update", this.onUpdate);
    clearTimeout(this.timer);
    this.listeners.clear();
  }

  /** Send what is waiting now, without the pause. */
  async flush(): Promise<void> {
    clearTimeout(this.timer);
    this.timer = undefined;
    await this.run();
  }

  /** Ask the server for what others wrote (the change feed said something changed). */
  async pull(): Promise<void> {
    if (this.stopped) {
      return;
    }
    await this.run();
  }

  private set(state: SaveState): void {
    if (state !== this.state) {
      this.state = state;
      for (const listener of this.listeners) {
        listener(state);
      }
    }
  }

  private schedule(): void {
    if (this.stopped) {
      return;
    }
    this.set("saving");
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.run();
    }, this.debounceMs);
  }

  private async run(): Promise<void> {
    if (this.stopped || this.state === "rejected") {
      return;
    }
    if (this.running) {
      this.again = true;
      return;
    }
    this.running = true;
    try {
      do {
        this.again = false;
        await this.once();
      } while (this.again && !this.stopped && this.state !== "rejected");
    } finally {
      this.running = false;
    }
  }

  private async once(): Promise<void> {
    const pending = Y.encodeStateAsUpdate(this.doc, this.server);
    const sending = !isEmpty(pending);
    try {
      const reply = await this.call({
        state_vector: toBase64(Y.encodeStateVector(this.doc)),
        ...(sending ? { update: toBase64(pending) } : {}),
      });
      this.failures = 0;
      Y.applyUpdate(this.doc, fromBase64(reply.update), REMOTE);
      this.server = fromBase64(reply.state_vector);
      // More was typed while the call was on its way.
      const left = Y.encodeStateAsUpdate(this.doc, this.server);
      if (isEmpty(left)) {
        this.set("saved");
      } else {
        this.again = true;
      }
    } catch (error) {
      if (this.refused(error)) {
        this.set("rejected");
        return;
      }
      this.set(sending || this.state !== "saved" ? "retrying" : this.state);
      const wait = this.retryMs[Math.min(this.failures, this.retryMs.length - 1)] ?? 1000;
      this.failures += 1;
      clearTimeout(this.timer);
      this.timer = setTimeout(() => {
        this.timer = undefined;
        void this.run();
      }, wait);
    }
  }
}

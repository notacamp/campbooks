/**
 * lib/logger — leveled client-side logger.
 *
 * This is the ONLY place in the app that is allowed to use console.* directly
 * (enforced by ESLint `no-console: error` everywhere except this file).
 * Modules must never call console.*; they call logger.* instead.
 *
 * In development, every level is emitted. In production, only warn and error
 * are emitted (debug/info are no-ops to save bandwidth and avoid leaking data).
 *
 * Sinks (e.g. an error-tracking service) are registered via registerLogSink().
 * Sinks receive every event regardless of the level filter, so an error tracker
 * can capture everything while the console stays quiet in production.
 *
 * NEVER log tokens, passwords, or PII. Client logs are visible in devtools and
 * may be forwarded to third-party services by registered sinks.
 */

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEvent {
  level: LogLevel;
  message: string;
  context?: Record<string, unknown>;
}

export type LogSink = (event: LogEvent) => void;

const sinks: LogSink[] = [];

const isDev =
  typeof import.meta !== "undefined" &&
  import.meta.env?.MODE === "development";

/** Register an additional log sink (e.g. observability/error-tracking). */
export const registerLogSink = (sink: LogSink): void => {
  sinks.push(sink);
};

const emit = (level: LogLevel, message: string, context?: Record<string, unknown>): void => {
  const event: LogEvent = { level, message, context };
  for (const sink of sinks) {
    sink(event);
  }
};

const shouldPrint = (level: LogLevel): boolean => {
  if (isDev) return true;
  return level === "warn" || level === "error";
};

const format = (level: LogLevel, message: string, context?: Record<string, unknown>): void => {
  if (!shouldPrint(level)) return;
  const prefix = `[campbooks:${level}]`;
  if (context !== undefined) {
    console[level](prefix, message, context);
  } else {
    console[level](prefix, message);
  }
};

export const logger = {
  debug: (message: string, context?: Record<string, unknown>): void => {
    format("debug", message, context);
    emit("debug", message, context);
  },
  info: (message: string, context?: Record<string, unknown>): void => {
    format("info", message, context);
    emit("info", message, context);
  },
  warn: (message: string, context?: Record<string, unknown>): void => {
    format("warn", message, context);
    emit("warn", message, context);
  },
  error: (message: string, context?: Record<string, unknown>): void => {
    format("error", message, context);
    emit("error", message, context);
  },
};

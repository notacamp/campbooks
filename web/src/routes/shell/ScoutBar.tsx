/**
 * ScoutBar — the persistent AI assistant input pinned to the bottom of main.
 *
 * ⌘K focuses the input. Submitting opens a stubbed answer popover above the bar.
 * On mobile (< 680px) the bar sits above the bottom tab bar.
 */
import { type FC, useState, useRef, useCallback, useEffect } from "react";
import { Send } from "lucide-react";
import { Visor } from "~/lib/ui";
import { cn } from "~/lib/utils";

interface ScoutBarProps {
  /** Extra class for the wrapping element (used for mobile stacking). */
  className?: string;
}

export const ScoutBar: FC<ScoutBarProps> = ({ className }) => {
  const [value, setValue] = useState("");
  const [answer, setAnswer] = useState<string | null>(null);
  const [thinking, setThinking] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // ⌘K / Ctrl+K → focus
  useEffect(() => {
    const handler = (e: KeyboardEvent): void => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const submit = useCallback((): void => {
    const q = value.trim();
    if (!q) return;
    setThinking(true);
    setAnswer(null);
    setValue("");
    // Stubbed response — real Scout chat is wired in the scout module.
    setTimeout(() => {
      setThinking(false);
      setAnswer("Scout is available — the AI chat module is coming next.");
    }, 800);
  }, [value]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>): void => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        submit();
      }
      if (e.key === "Escape") {
        setAnswer(null);
        inputRef.current?.blur();
      }
    },
    [submit],
  );

  return (
    <div className={cn("relative", className)}>
      {/* Answer popover — sits above the bar */}
      {(answer != null || thinking) && (
        <div
          role="status"
          aria-live="polite"
          className={cn(
            "absolute bottom-full left-0 right-0 mb-2 mx-3",
            "bg-surface-2 cb-overlay rounded-cb-3 p-4",
            "text-[13px] text-t2 leading-relaxed",
          )}
        >
          {thinking ? (
            <span className="flex items-center gap-2 text-t3">
              <Visor state="thinking" size={18} label="Scout thinking" />
              Scout is thinking…
            </span>
          ) : (
            <span>{answer}</span>
          )}
        </div>
      )}

      {/* Input row */}
      <div
        className={cn(
          "flex items-center gap-2 px-3 py-2 border-t border-line bg-ground",
        )}
      >
        <Visor
          state={thinking ? "thinking" : "watching"}
          size={18}
          gaze={!thinking}
          label="Scout"
          className="shrink-0 opacity-70"
        />
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask Scout… (⌘K)"
          className={cn(
            "flex-1 bg-transparent text-[13px] text-t1 placeholder:text-t4",
            "outline-none border-none",
          )}
          aria-label="Ask Scout"
        />
        <button
          type="button"
          onClick={submit}
          disabled={!value.trim() || thinking}
          aria-label="Send"
          className={cn(
            "shrink-0 w-7 h-7 flex items-center justify-center rounded-cb-1",
            "text-t4 hover:text-t2 hover:bg-surface-1",
            "transition-colors duration-[--cb-dur]",
            "disabled:opacity-40 disabled:pointer-events-none",
          )}
        >
          <Send size={14} strokeWidth={2} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
};

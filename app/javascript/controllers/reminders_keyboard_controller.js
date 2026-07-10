import { Controller } from "@hotwired/stimulus"

// Keyboard navigation for the /reminders list. A discrete cursor (unlike the
// scroll-focused home feed): j/k or the arrows move [data-focused] through the
// rows, and the focused row's own buttons are clicked for the actions — so each
// key rides the exact Turbo path a tap takes (toast + turbo_stream.remove of the
// row), and the cursor then advances to the next reminder.
//
//   j / ArrowDown   next reminder        k / ArrowUp   previous reminder
//   Enter           Add to calendar (or the confirmed row's View event)
//   s               Snooze               d             Dismiss
//   ?               open the global shortcuts modal
//
// Honors typing context, modifier keys and open dialogs. Rows are re-queried on
// every keypress so turbo-stream changes are picked up automatically. A no-op
// without a keyboard — mobile drives the same actions through the on-row buttons.
export default class extends Controller {
  connect() {
    this.focused = null
    this.onKey = this.handleKey.bind(this)
    this.onStream = this.handleStream.bind(this)
    document.addEventListener("keydown", this.onKey)
    document.addEventListener("turbo:before-stream-render", this.onStream)
  }

  disconnect() {
    document.removeEventListener("keydown", this.onKey)
    document.removeEventListener("turbo:before-stream-render", this.onStream)
  }

  handleKey(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    const target = event.target
    if (target instanceof Element && target.closest("input, textarea, select, [contenteditable='true']")) return
    // A native <dialog> (e.g. the shortcuts modal) owns the keyboard while open.
    if (document.querySelector("dialog[open]")) return
    if (this.rows().length === 0) return

    switch (event.key) {
      case "j": case "ArrowDown": this.move(1); event.preventDefault(); break
      case "k": case "ArrowUp":   this.move(-1); event.preventDefault(); break
      case "Enter": if (this.act("confirm") || this.act("primary")) event.preventDefault(); break
      case "s": if (this.act("snooze")) event.preventDefault(); break
      case "d": if (this.act("dismiss")) event.preventDefault(); break
      case "?": this.showHelp(); event.preventDefault(); break
    }
  }

  rows() {
    return Array.from(this.element.querySelectorAll("[data-reminders-row]"))
  }

  // The row under the cursor. Falls back to the flagged row (survives a re-render
  // that dropped our reference), and is null before the first move.
  active() {
    if (this.focused && this.focused.isConnected) return this.focused
    return this.element.querySelector("[data-reminders-row][data-focused]") || null
  }

  // Step the cursor. With no cursor yet, j/↓ lands on the first row and k/↑ on the
  // last; otherwise clamp at the ends (no wrap).
  move(delta) {
    const rows = this.rows()
    const current = this.active()
    const i = current
      ? Math.max(0, Math.min(rows.length - 1, rows.indexOf(current) + delta))
      : (delta > 0 ? 0 : rows.length - 1)
    const next = rows[i]
    if (!next) return
    this.setFocus(next)
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    next.scrollIntoView({ behavior: reduce ? "auto" : "smooth", block: "center" })
  }

  setFocus(row) {
    if (this.focused === row) return
    this.focused?.removeAttribute("data-focused")
    row?.setAttribute("data-focused", "")
    this.focused = row || null
  }

  // Click the focused row's own action button, reusing its exact Turbo path.
  // Returns true if one was clicked (so the caller can preventDefault).
  act(name) {
    const el = this.active()?.querySelector(`[data-reminders-action="${name}"]`)
    if (!el) return false
    el.click()
    return true
  }

  // When the focused row is about to be removed by its confirm/snooze/dismiss
  // stream, advance the cursor to the next (or previous) row so the reader keeps
  // their place. The sibling is captured before the removal, while it's still there.
  handleStream(event) {
    const stream = event.target
    if (!stream || stream.tagName !== "TURBO-STREAM" || stream.getAttribute("action") !== "remove") return
    const current = this.active()
    if (!current || stream.getAttribute("target") !== current.id) return

    const rows = this.rows()
    const idx = rows.indexOf(current)
    const next = rows[idx + 1] || rows[idx - 1] || null
    this.focused = null
    // Re-focus once the removal has rendered (this fires before the stream applies).
    setTimeout(() => { if (next && next.isConnected) this.setFocus(next) }, 0)
  }

  showHelp() {
    const dialog = document.getElementById("keyboard-shortcuts-modal")
    if (dialog && typeof dialog.showModal === "function" && !dialog.open) dialog.showModal()
  }
}

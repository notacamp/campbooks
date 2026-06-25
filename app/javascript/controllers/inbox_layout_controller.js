import { Controller } from "@hotwired/stimulus"

// Switches the inbox between LAYOUT modes:
//   "default" — the multi-pane shell (folder rail | thread list | reading frame | discussion)
//   "list"    — a full-width list; clicking a row opens the bottom-right email drawer
//   "board"   — a status kanban (Inbox / Snoozed / Awaiting reply / Done)
//
// This is orthogonal to the DENSITY "view mode" (compact/default/breathable),
// which is owned by the inbox-settings-modal controller. We persist the choice
// in localStorage so it survives reloads and full-page folder navigations, and
// re-apply it on connect (the server always renders the default shell).
const STORAGE_KEY = "inbox_layout"
// The valid layouts are derived at runtime from the rendered switcher buttons
// (see `get layouts()`), not hardcoded: the server omits the Board segment when
// its feature flag is off, and a stale saved "board" then falls back to "default".

export default class extends Controller {
  static targets = ["button", "boardFrame"]
  static values = { boardSrc: String }

  connect() {
    // A drawer "Discussion" deep-link (#discussion) opens the focused multi-pane
    // reading view even when List/Board is the saved layout — the reading +
    // discussion panes are hidden in those layouts (see application.css). One-shot:
    // we don't persist it, so navigating back to the inbox keeps the chosen layout.
    this.apply(this.requestsFocusView() ? "default" : this.current)
  }

  requestsFocusView() {
    return window.location.hash === "#discussion"
  }

  // The Board layout renders its own copy of the switcher inside a lazily-loaded
  // turbo-frame (the list pane that holds the primary switcher is hidden in Board
  // mode). Those buttons connect after apply() has run, so sync them on arrival
  // to reflect the active segment.
  buttonTargetConnected() {
    this._syncButtons(this.current)
  }

  // Layouts actually on offer = whatever the server rendered as switcher buttons.
  // Falls back to ["default"] if the buttons haven't connected yet.
  get layouts() {
    const fromButtons = this.buttonTargets.map((b) => b.dataset.layout)
    return fromButtons.length ? fromButtons : ["default"]
  }

  get current() {
    const saved = localStorage.getItem(STORAGE_KEY)
    return this.layouts.includes(saved) ? saved : "default"
  }

  select(event) {
    const layout = event.currentTarget.dataset.layout
    if (!this.layouts.includes(layout)) return
    localStorage.setItem(STORAGE_KEY, layout)
    this.apply(layout)
  }

  apply(layout) {
    this.element.setAttribute("data-inbox-layout", layout)
    this._syncButtons(layout)
    if (layout === "board") this._loadBoard()
  }

  // Lazily point the board turbo-frame at its endpoint the first time Board is
  // selected; subsequent switches reuse the already-loaded frame.
  _loadBoard() {
    if (!this.hasBoardFrameTarget) return
    const frame = this.boardFrameTarget
    if (!frame.getAttribute("src") && this.boardSrcValue) {
      frame.setAttribute("src", this.boardSrcValue)
    }
  }

  _syncButtons(layout) {
    this.buttonTargets.forEach((btn) => {
      const active = btn.dataset.layout === layout
      btn.setAttribute("aria-pressed", active ? "true" : "false")
      btn.classList.toggle("bg-card", active)
      btn.classList.toggle("text-foreground", active)
      btn.classList.toggle("shadow-sm", active)
      btn.classList.toggle("text-muted-foreground", !active)
    })
  }
}

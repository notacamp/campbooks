import { Controller } from "@hotwired/stimulus"

// Switches the Files list between LAYOUT modes:
//   "list" — the current table (md+) / stacked cards (below md)
//   "grid" — thumbnail tiles at every width
//
// Mirrors inbox_layout_controller: the choice persists in localStorage so it
// survives reloads and folder navigation, and is re-applied on connect (the
// server always renders the list layout; application.css swaps the panes based
// on the data-files-layout attribute this controller sets).
const STORAGE_KEY = "files_layout"

export default class extends Controller {
  static targets = ["button"]

  connect() {
    this.apply(this.current)
  }

  // Layouts on offer = whatever the server rendered as switcher buttons.
  get layouts() {
    const fromButtons = this.buttonTargets.map((b) => b.dataset.layout)
    return fromButtons.length ? fromButtons : ["list"]
  }

  get current() {
    const saved = localStorage.getItem(STORAGE_KEY)
    return this.layouts.includes(saved) ? saved : "list"
  }

  select(event) {
    const layout = event.currentTarget.dataset.layout
    if (!this.layouts.includes(layout)) return
    localStorage.setItem(STORAGE_KEY, layout)
    this.apply(layout)
  }

  apply(layout) {
    this.element.setAttribute("data-files-layout", layout)
    this._syncButtons(layout)
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

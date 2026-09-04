import { Controller } from "@hotwired/stimulus"
import { skimOverlayOpen } from "controllers/skim_utils"

// Keyboard shortcuts for the People list pane.
//
// ↑ / ↓  — move aria-selected between [data-people-row] rows (all lanes + Recent,
//           DOM order), scroll into view, open after 150 ms debounce.
// Enter  — open the selected row immediately.
// r      — click the selected row's Reply form button.
// e      — click the selected row's Done form button.
// s      — click the selected row's Snooze form button.
// .      — open the selected row's More <details>.
// Escape — clear the selection.
//
// Guards: ignored when focus is inside an editable, when the command palette or
// Scout overlay is open, and on touch-only devices.

const EDITABLE_SELECTOR = "input, textarea, select, [contenteditable], [role=textbox]"
const OPEN_DEBOUNCE_MS = 150

export default class extends Controller {
  static targets = ["list"]

  connect () {
    // Skip on touch-only devices (no hover capability).
    if (window.matchMedia("(hover: none)").matches) return

    this._bound = this._keydown.bind(this)
    window.addEventListener("keydown", this._bound)
    this._debounceTimer = null
  }

  disconnect () {
    window.removeEventListener("keydown", this._bound)
    clearTimeout(this._debounceTimer)
  }

  // ── Private ─────────────────────────────────────────────────────────────────

  _keydown (event) {
    if (this._shouldIgnore(event)) return

    switch (event.key) {
      case "ArrowUp":
        event.preventDefault()
        this._moveSelection(-1)
        break
      case "ArrowDown":
        event.preventDefault()
        this._moveSelection(1)
        break
      case "Enter":
        event.preventDefault()
        this._openSelected()
        break
      case "r":
        this._clickAction("[data-people-reply]")
        break
      case "e":
        this._clickAction("[data-people-done]")
        break
      case "s":
        this._clickAction("[data-people-snooze]")
        break
      case ".":
        this._clickAction("[data-people-more]")
        break
      case "Escape":
        this._clearSelection()
        break
    }
  }

  _shouldIgnore (event) {
    // Respect command palette.
    const palette = document.querySelector(".command-palette-dialog[open]")
    if (palette) return true

    // Respect Scout/skim overlay.
    if (skimOverlayOpen()) return true

    // Don't intercept typing in editable fields.
    const el = document.activeElement
    if (el && (el.matches(EDITABLE_SELECTOR) || el.closest(EDITABLE_SELECTOR))) return true

    // Only handle unmodified keys.
    if (event.metaKey || event.ctrlKey || event.altKey) return true

    return false
  }

  _rows () {
    const listEl = this.hasListTarget ? this.listTarget : this.element
    return Array.from(listEl.querySelectorAll("[data-people-row]"))
  }

  _selectedRow () {
    return this._rows().find(r => r.getAttribute("aria-selected") === "true") || null
  }

  _moveSelection (delta) {
    const rows = this._rows()
    if (rows.length === 0) return

    const current = this._selectedRow()
    const idx = current ? rows.indexOf(current) : -1
    const next = rows[Math.max(0, Math.min(rows.length - 1, idx + delta))]
    if (!next || next === current) return

    this._select(next)
    next.scrollIntoView({ block: "nearest" })

    // Debounced open.
    clearTimeout(this._debounceTimer)
    this._debounceTimer = setTimeout(() => this._openRow(next), OPEN_DEBOUNCE_MS)
  }

  _select (row) {
    this._rows().forEach(r => r.removeAttribute("aria-selected"))
    row.setAttribute("aria-selected", "true")
  }

  _clearSelection () {
    this._rows().forEach(r => r.removeAttribute("aria-selected"))
  }

  _openSelected () {
    const row = this._selectedRow()
    if (row) this._openRow(row)
  }

  _openRow (row) {
    const link = row.querySelector("a[data-turbo-frame='people_detail']")
    if (link) link.click()
  }

  // Click the first matching action button inside the selected row.
  _clickAction (selector) {
    const row = this._selectedRow()
    if (!row) return
    const btn = row.querySelector(selector)
    if (btn) btn.click()
  }
}

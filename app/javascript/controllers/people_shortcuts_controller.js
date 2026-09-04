import { Controller } from "@hotwired/stimulus"
import { skimOverlayOpen } from "controllers/skim_utils"

// Keyboard shortcuts for the People list pane and open conversation.
//
// ↑ / ↓  — move aria-selected between [data-people-row] rows (all lanes + Recent,
//           DOM order), scroll into view, open after 150 ms debounce.
// Enter  — open the selected row immediately.
// r      — click the selected row's Reply form button; falls back to the first
//           [data-people-reply] inside #people_detail when no row is selected.
// a      — click the first [data-people-reply-all] inside #people_detail.
// e      — click the selected row's Done form button.
// s      — click the selected row's Snooze form button.
// f      — click the first [data-people-forward] inside #people_detail.
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
      case "r": {
        const replied = this._clickAction("[data-people-reply]")
        if (!replied) this._clickDetail("[data-people-reply]")
        break
      }
      case "a":
        this._clickDetail("[data-people-reply-all]")
        break
      case "e":
        this._clickAction("[data-people-done]")
        break
      case "s":
        this._clickAction("[data-people-snooze]")
        break
      case "f":
        this._clickDetail("[data-people-forward]")
        break
      case ".":
        this._clickAction("[data-people-more]")
        break
      case "i":
        this._toggleDetails()
        break
      case "Escape":
        this._clearSelection()
        break
    }
  }

  _shouldIgnore (event) {
    // Respect any open dialog — the Scout overlay (⌘K) above all: its arrow keys
    // move through ITS results, not the People list behind it.
    if (document.querySelector("dialog[open]")) return true

    // Respect the skim overlay (a role="dialog" panel, not a native <dialog>).
    if (skimOverlayOpen()) return true

    // Don't intercept typing in editable fields.
    const el = document.activeElement
    if (el && (el.matches(EDITABLE_SELECTOR) || el.closest(EDITABLE_SELECTOR))) return true

    // Enter on a focused control activates that control, not the selected row.
    if (event.key === "Enter" && el && el.matches("button, a, summary, select, [role=button]")) return true

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
  // Returns true when a button was found and clicked.
  _clickAction (selector) {
    const row = this._selectedRow()
    if (!row) return false
    const btn = row.querySelector(selector)
    if (btn) { btn.click(); return true }
    return false
  }

  // Click the first matching button inside #people_detail (the open conversation).
  _clickDetail (selector) {
    const detail = document.getElementById("people_detail")
    if (!detail) return
    const btn = detail.querySelector(selector)
    if (btn) btn.click()
  }

  // Toggle the Details sheet (below xl) or collapse/expand the rail (xl+).
  _toggleDetails () {
    const detailPane = document.getElementById("people_details_pane")
    if (!detailPane) return
    // Find the people-details Stimulus controller on the conversation pane parent.
    const wrapper = detailPane.closest("[data-controller~='people-details']")
    if (wrapper) {
      const app = this.application.getControllerForElementAndIdentifier(wrapper, "people-details")
      if (app && typeof app.toggle === "function") app.toggle()
    } else {
      // Fallback: toggle class directly.
      detailPane.classList.toggle("translate-x-0")
      detailPane.classList.toggle("translate-x-full")
    }
  }
}

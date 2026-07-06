import { Controller } from "@hotwired/stimulus"

// Gmail-style two-step navigation shortcuts for the left nav rail.
//
//   g           arm navigation mode (3 s timeout; Escape or any click disarms)
//   g h         Home
//   g m         Mail
//   g c         Calendar
//   g s         Scout
//   g f         Files
//   g t         Tasks
//   g d         Digests
//   g w         Workflows
//   g p         Contacts (People)
//   g o         Organizations
//   g a         Activity
//
// While armed, body[data-nav-armed] is set, which the CSS uses to reveal the
// small key-badge chips inside each nav item. A second keypress navigates and
// disarms; anything outside the map also disarms (no silent swallowing).
//
// Runs in the document capture phase so it intercepts keydowns before the
// calendar-nav, email-shortcuts, and feed-keyboard controllers when armed,
// stopping propagation only on the consumed key.
//
// Guards: modifier keys, editable fields, and open dialogs (including the
// command palette) all suppress both the arm and the navigate steps, matching
// the convention in email-shortcuts and calendar-nav.

const EDITABLE_SELECTOR = "input, textarea, select, [contenteditable], [role=textbox]"
const ARM_TIMEOUT_MS = 3000

export default class extends Controller {
  connect() {
    this._armed = false
    this._timer = null
    this._boundCapture = this._onKeydown.bind(this)
    // Capture phase: fired before bubble-phase listeners (calendar-nav, email-shortcuts…).
    document.addEventListener("keydown", this._boundCapture, true)
    // Disarm on any click (navigating away or closing a menu counts as disarm).
    this._boundClick = this._disarm.bind(this)
    document.addEventListener("click", this._boundClick)
    // Disarm on Turbo navigation (keeps the badge state clean across pages).
    this._boundVisit = this._disarm.bind(this)
    document.addEventListener("turbo:before-visit", this._boundVisit)
  }

  disconnect() {
    document.removeEventListener("keydown", this._boundCapture, true)
    document.removeEventListener("click", this._boundClick)
    document.removeEventListener("turbo:before-visit", this._boundVisit)
    this._disarm()
  }

  _onKeydown(event) {
    // Never intercept modified keys (Cmd/Ctrl/Alt combos — system or palette).
    if (event.metaKey || event.ctrlKey || event.altKey) return

    // Never fire while typing in an editable field.
    const el = document.activeElement
    if (el && el.matches(EDITABLE_SELECTOR)) return

    // Never fire while any dialog (modal, command palette) is open.
    if (document.querySelector("dialog[open]")) return

    if (!this._armed) {
      // Arm on bare 'g'.
      if (event.key === "g") {
        event.preventDefault()
        event.stopImmediatePropagation()
        this._arm()
      }
      // Any other key: don't consume, let other controllers handle it.
      return
    }

    // ── Armed: handle the second key ────────────────────────────────────────
    // Always consume the keydown so calendar-nav / email-shortcuts don't also
    // react to c/a/r/e/arrows while we hold the nav chord.
    event.preventDefault()
    event.stopImmediatePropagation()

    if (event.key === "Escape") {
      this._disarm()
      return
    }

    // Find the nav link for this key. Multiple links may carry the same
    // data attribute (NavRail + BottomNav); prefer the one currently visible
    // (offsetParent !== null ≈ display:block subtree).
    const links = Array.from(
      document.querySelectorAll(`[data-nav-shortcut-key="${event.key}"]`)
    )
    const link = links.find(el => el.offsetParent !== null) || links[0]

    this._disarm()

    if (link) {
      link.click()
    }
    // Unknown key: disarm silently (already done above).
  }

  _arm() {
    this._armed = true
    document.body.dataset.navArmed = "true"
    this._timer = setTimeout(() => this._disarm(), ARM_TIMEOUT_MS)
  }

  _disarm() {
    this._armed = false
    delete document.body.dataset.navArmed
    if (this._timer) {
      clearTimeout(this._timer)
      this._timer = null
    }
  }
}

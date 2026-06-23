import { Controller } from "@hotwired/stimulus"

// Google-Calendar-style keyboard navigation for the calendar page. Mounted on the
// calendar root; uses a document keydown listener (like feed-keyboard) so keys work
// without a click, scoped to this page by the controller's connect/disconnect.
//
//   t            jump to today          d / w / m / a   day / week / month / agenda
//   j / →        next period            c               new event (opens the modal)
//   k / ←        previous period        ?               keyboard shortcuts help
//
// Navigation + view keys click the header's existing prev/next/today links and the
// ViewTabs (data-calendar-{prev,next,today,view}), reusing their server-computed
// URLs. While connected it sets body.dataset.calendarKeys so the global
// email-shortcuts controller stands aside (no c/a/r/e/arrow collisions).
export default class extends Controller {
  connect() {
    this.onKey = this._handleKey.bind(this)
    document.addEventListener("keydown", this.onKey)
    document.body.dataset.calendarKeys = "1"
  }

  disconnect() {
    document.removeEventListener("keydown", this.onKey)
    delete document.body.dataset.calendarKeys
  }

  _handleKey(event) {
    if (this._shouldIgnore(event)) return

    // "?" help — match the resolved char and Shift+/ (keyboard-layout safe, mirrors
    // email-shortcuts).
    if (event.key === "?" || (event.shiftKey && event.key === "/")) {
      event.preventDefault()
      this._showHelp()
      return
    }

    switch (event.key) {
      case "t": this._click("[data-calendar-today]"); break
      case "j": case "ArrowRight": this._click("[data-calendar-next]"); break
      case "k": case "ArrowLeft": this._click("[data-calendar-prev]"); break
      case "d": this._view("day"); break
      case "w": this._view("week"); break
      case "m": this._view("month"); break
      case "a": this._view("agenda"); break
      case "c": this._newEvent(); break
      default: return
    }
    event.preventDefault()
  }

  // Bail on modifier combos (reserved for the palette/system), while typing in a
  // field, and whenever any dialog is open (modal/help/palette own their own keys).
  _shouldIgnore(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return true
    const target = event.target
    if (target instanceof Element && target.closest("input, textarea, select, [contenteditable], [role=textbox]")) return true
    if (document.querySelector("dialog[open]")) return true
    return false
  }

  _click(selector) {
    document.querySelector(selector)?.click()
  }

  _view(view) {
    document.querySelector(`[data-calendar-view="${view}"]`)?.click()
  }

  // Open the create modal for the current view (mirrors the header "New event"
  // button); the calendar-event-modal controller listens for this event.
  _newEvent() {
    const view = new URLSearchParams(window.location.search).get("view")
    window.dispatchEvent(new CustomEvent("calendar-event-modal:open", {
      detail: { url: `/calendar_events/new${view ? `?view=${encodeURIComponent(view)}` : ""}` }
    }))
  }

  _showHelp() {
    document.getElementById("keyboard-shortcuts-modal")?.showModal()
  }
}

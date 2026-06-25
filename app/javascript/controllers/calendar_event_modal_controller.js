import { Controller } from "@hotwired/stimulus"

// Hosts the create/edit calendar-event form in a modal so creating an event never
// leaves the calendar. Mirrors setup-modal: a native <dialog> wrapping a Turbo
// Frame whose src we point at /calendar_events/new|:id/edit on open.
//
// Opens from three sources, all funnelling into open(url):
//   - a click on any [data-calendar-event-modal-open="<url>"] (header button,
//     month-grid day, week "add" link, event chips for edit)
//   - the `calendar-event-modal:open` window event (grid click/drag + the `c` key)
//   - a ?new_event=1 deep-link (Cmd+K "New calendar event" navigates here)
// Escape + backdrop close come from the native <dialog>.
export default class extends Controller {
  static targets = ["dialog", "frame"]

  connect() {
    this._onClick = this._handleClick.bind(this)
    this._onOpen = this._handleOpenEvent.bind(this)
    this._onDialogClick = this._handleDialogClick.bind(this)
    this._onClose = this._handleClose.bind(this)

    document.addEventListener("click", this._onClick)
    window.addEventListener("calendar-event-modal:open", this._onOpen)
    this.dialogTarget.addEventListener("click", this._onDialogClick)
    this.dialogTarget.addEventListener("close", this._onClose)

    this._maybeAutoOpen()
  }

  disconnect() {
    document.removeEventListener("click", this._onClick)
    window.removeEventListener("calendar-event-modal:open", this._onOpen)
  }

  // Cmd+K "New calendar event" navigates to /calendar?new_event=1 — open the
  // new-event form, then strip the param so a refresh/back doesn't reopen it.
  _maybeAutoOpen() {
    const params = new URLSearchParams(window.location.search)
    if (params.get("new_event") !== "1") return
    const view = params.get("view")
    this.open(`/calendar_events/new${view ? `?view=${encodeURIComponent(view)}` : ""}`)
    this._stripParam("new_event")
  }

  _handleClick(event) {
    const trigger = event.target.closest("[data-calendar-event-modal-open]")
    if (!trigger) return
    // Let modifier/middle clicks fall through to the link's href (open in a tab).
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.button === 1) return
    event.preventDefault()
    this.open(trigger.getAttribute("data-calendar-event-modal-open"))
  }

  _handleOpenEvent(event) {
    const url = event.detail && event.detail.url
    if (url) this.open(url)
  }

  open(url) {
    if (!url) return
    this.frameTarget.setAttribute("src", url)
    if (!this.dialogTarget.open) this.dialogTarget.showModal()
  }

  // The close/cancel controls are links to the calendar (so they still work on the
  // standalone full-page form, where this controller isn't mounted). When the modal
  // IS present we handle the close in-place: swallow the click so it doesn't navigate.
  close(event) {
    if (event) event.preventDefault()
    if (this.dialogTarget.open) this.dialogTarget.close()
  }

  _handleDialogClick(event) {
    // A click that lands on the <dialog> itself (not its children) is the backdrop.
    if (event.target === this.dialogTarget) this.close()
  }

  // Reset the frame on close so the next open always refetches (fresh prefill or a
  // cleared error state) — setting src back to a value Turbo sees as changed.
  _handleClose() {
    this.frameTarget.setAttribute("src", "")
  }

  _stripParam(name) {
    const url = new URL(window.location.href)
    url.searchParams.delete(name)
    history.replaceState(history.state, "", url.toString())
  }
}

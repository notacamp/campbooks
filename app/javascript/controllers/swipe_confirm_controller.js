import { Controller } from "@hotwired/stimulus"

// Shared confirm dialog for destructive swipe actions (e.g. permanent delete).
// The swipe-actions controller dispatches `swipe-actions:confirm-request`
// { id, title, message, confirmLabel } and awaits `swipe-actions:confirm-response`
// { id, confirmed }. Using the native <dialog> `close` event funnels button,
// Escape and backdrop dismissals through one response path.
export default class extends Controller {
  static targets = ["title", "message", "confirm", "remember", "rememberRow"]

  connect() {
    this._onRequest = (e) => this._open(e.detail)
    window.addEventListener("swipe-actions:confirm-request", this._onRequest)
    this.element.addEventListener("close", () => this._respond())
  }

  disconnect() {
    window.removeEventListener("swipe-actions:confirm-request", this._onRequest)
  }

  _open({ id, title, message, confirmLabel, rememberKey, color }) {
    this._id = id
    this._confirmed = false
    if (title && this.hasTitleTarget) this.titleTarget.textContent = title
    if (message && this.hasMessageTarget) this.messageTarget.textContent = message
    if (confirmLabel && this.hasConfirmTarget) this.confirmTarget.textContent = confirmLabel
    // Tint the confirm button with the swiped action's own accent (falls back to red).
    if (this.hasConfirmTarget) this.confirmTarget.style.setProperty("--swipe-accent", color ? `var(--swipe-${color})` : "")
    // Offer "Don't ask again" only when the action opted in (has a rememberKey).
    if (this.hasRememberTarget) this.rememberTarget.checked = false
    if (this.hasRememberRowTarget) this.rememberRowTarget.style.display = rememberKey ? "flex" : "none"
    this.element.showModal()
  }

  confirm() { this._confirmed = true; this.element.close() }
  cancel() { this._confirmed = false; this.element.close() }
  backdrop(e) { if (e.target === this.element) { this._confirmed = false; this.element.close() } }

  _respond() {
    if (!this._id) return
    const id = this._id
    this._id = null
    const dontAskAgain = this.hasRememberTarget && this.rememberTarget.checked
    window.dispatchEvent(new CustomEvent("swipe-actions:confirm-response", {
      detail: { id, confirmed: this._confirmed, dontAskAgain }
    }))
  }
}

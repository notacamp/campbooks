import { Controller } from "@hotwired/stimulus"

// Shared snooze-time picker for the email swipe "Snooze" stage. The swipe-actions
// controller dispatches `swipe-actions:snooze-request` { id } and awaits
// `swipe-actions:snooze-response` { id, value } where value is an ISO 8601 string
// or null (cancelled). Presets are server-rendered (data-snooze-until carries the
// app-timezone iso8601), so there's no time math here.
export default class extends Controller {
  static targets = ["custom"]

  connect() {
    this._onRequest = (e) => this._open(e.detail)
    window.addEventListener("swipe-actions:snooze-request", this._onRequest)
    this.element.addEventListener("close", () => this._respond())
  }

  disconnect() {
    window.removeEventListener("swipe-actions:snooze-request", this._onRequest)
  }

  _open({ id }) {
    this._id = id
    this._value = null
    if (this.hasCustomTarget) this.customTarget.value = ""
    this.element.showModal()
  }

  pick(e) {
    this._value = e.currentTarget.dataset.snoozeUntil || null
    this.element.close()
  }

  pickCustom() {
    const raw = this.hasCustomTarget ? this.customTarget.value : ""
    if (!raw) return
    this._value = new Date(raw).toISOString()
    this.element.close()
  }

  cancel() { this._value = null; this.element.close() }
  backdrop(e) { if (e.target === this.element) { this._value = null; this.element.close() } }

  _respond() {
    if (!this._id) return
    const id = this._id
    this._id = null
    window.dispatchEvent(new CustomEvent("swipe-actions:snooze-response", {
      detail: { id, value: this._value }
    }))
  }
}

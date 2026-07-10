import { Controller } from "@hotwired/stimulus"

// Polls a rule row's Turbo Frame while a retroactive run is queued/running.
// The frame is the controller's element; calling reload() re-fetches its src.
// Turbo Frame disconnects (removes from DOM) when the row is replaced with the
// completed state, which triggers disconnect() and stops the interval.
export default class extends Controller {
  static values = { url: String }

  connect() {
    this._timer = setInterval(() => this._reload(), 2500)
  }

  disconnect() {
    clearInterval(this._timer)
  }

  _reload() {
    // this.element is the <turbo-frame> element
    if (typeof this.element.reload === "function") {
      this.element.reload()
    }
  }
}

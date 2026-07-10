import { Controller } from "@hotwired/stimulus"

// Polls a rule row's Turbo Frame while a retroactive run is queued/running.
//
// The frame is rendered WITHOUT a src (a frame whose response src references
// itself is rejected by Turbo), so the first tick assigns src from the url
// value (which navigates the frame) and later ticks call reload().
//
// Frame navigation only swaps the frame's children, so this controller stays
// attached across polls; it stops itself when a poll comes back without the
// [data-rule-run-progress] marker (run finished). A turbo_stream.replace of
// the whole row (run/undo actions) removes the element and disconnect() also
// stops the interval.
export default class extends Controller {
  static values = { url: String }

  connect() {
    this._onFrameLoad = () => this._stopIfFinished()
    this.element.addEventListener("turbo:frame-load", this._onFrameLoad)
    this._timer = setInterval(() => this._poll(), 2500)
  }

  disconnect() {
    this._stop()
  }

  _poll() {
    const frame = this.element
    if (frame.getAttribute("src")) {
      if (typeof frame.reload === "function") frame.reload()
    } else {
      frame.src = this.urlValue
    }
  }

  _stopIfFinished() {
    if (!this.element.querySelector("[data-rule-run-progress]")) this._stop()
  }

  _stop() {
    clearInterval(this._timer)
    if (this._onFrameLoad) {
      this.element.removeEventListener("turbo:frame-load", this._onFrameLoad)
      this._onFrameLoad = null
    }
  }
}

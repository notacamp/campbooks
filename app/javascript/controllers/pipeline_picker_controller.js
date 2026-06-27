import { Controller } from "@hotwired/stimulus"

// The "add items to pipeline" modal. Loaded into the pipeline_picker Turbo
// Frame; closing removes the overlay (emptying the frame). The search field
// debounce-submits its GET form back into the same frame to filter the list.
export default class extends Controller {
  static targets = ["search"]

  connect() {
    if (this.hasSearchTarget) {
      // Defer so the frame is painted before stealing focus.
      requestAnimationFrame(() => this.searchTarget.focus())
    }
  }

  close() {
    this.element.remove()
  }

  backdropClose(event) {
    if (event.target === event.currentTarget) this.close()
  }

  keydown(event) {
    if (event.key === "Escape") this.close()
  }

  search() {
    clearTimeout(this._timer)
    this._timer = setTimeout(() => {
      this.searchTarget.form?.requestSubmit()
    }, 250)
  }
}

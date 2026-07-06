import { Controller } from "@hotwired/stimulus"

// Debounced live search for a list backed by a Turbo Frame. The <form> targets
// the results frame (data-turbo-frame), so this controller only decides *when*
// to submit: debounced on input, immediately on Enter or Clear. Generic —
// reusable by any single-field search form. The Clear button lives here (not a
// server-rendered link) because the form sits outside the frame it navigates, so
// it never re-renders on a search; JS keeps the button in sync with the field.
export default class extends Controller {
  static targets = ["input", "clear"]
  static values = { debounce: { type: Number, default: 250 } }

  connect() {
    this.toggleClear()
  }

  submit() {
    this.toggleClear()
    clearTimeout(this.timer)
    this.timer = setTimeout(() => this.element.requestSubmit(), this.debounceValue)
  }

  submitNow(event) {
    if (event) event.preventDefault()
    this.toggleClear()
    clearTimeout(this.timer)
    this.element.requestSubmit()
  }

  clear() {
    if (this.hasInputTarget) {
      this.inputTarget.value = ""
      this.inputTarget.focus()
    }
    this.submitNow()
  }

  // Show the Clear button only when there's something to clear.
  toggleClear() {
    if (!this.hasClearTarget) return
    const empty = !this.hasInputTarget || this.inputTarget.value.trim() === ""
    this.clearTarget.classList.toggle("hidden", empty)
  }

  disconnect() {
    clearTimeout(this.timer)
  }
}

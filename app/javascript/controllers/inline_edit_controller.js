import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["display", "form", "input"]

  connect() {
    this.submitting = false
  }

  edit() {
    this.displayTarget.classList.add("hidden")
    this.formTarget.classList.remove("hidden")
    this.inputTarget.focus()
    this.inputTarget.select()
  }

  cancel() {
    if (this.submitting) return
    this.formTarget.classList.add("hidden")
    this.displayTarget.classList.remove("hidden")
  }

  markSubmitting() {
    this.submitting = true
  }

  saved() {
    this.submitting = false
  }

  submitOnEnter(event) {
    if (event.key === "Enter") {
      event.preventDefault()
      this.submitting = true
      this.formTarget.requestSubmit()
    }
  }
}

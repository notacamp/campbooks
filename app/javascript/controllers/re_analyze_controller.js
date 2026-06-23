import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["button", "form"]

  toggle() {
    this.buttonTarget.classList.add("hidden")
    this.formTarget.classList.remove("hidden")
    this.formTarget.querySelector("textarea")?.focus()
  }

  cancel() {
    this.formTarget.classList.add("hidden")
    this.buttonTarget.classList.remove("hidden")
  }

  submitting() {
    const textarea = this.formTarget.querySelector("textarea")
    const submitBtn = this.formTarget.querySelector("button[type=submit]")
    if (textarea) textarea.disabled = true
    if (submitBtn) {
      submitBtn.disabled = true
      submitBtn.textContent = "Queuing..."
    }
  }
}

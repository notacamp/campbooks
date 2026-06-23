import { Controller } from "@hotwired/stimulus"

// Copies the source element's value (or text) to the clipboard and briefly
// confirms on the trigger button.
export default class extends Controller {
  static targets = ["source", "button"]
  static values = { successText: { type: String, default: "Copied!" } }

  copy() {
    const source = this.sourceTarget
    const text = source.value ?? source.textContent ?? ""

    navigator.clipboard.writeText(text).then(() => this.confirm())
  }

  confirm() {
    if (!this.hasButtonTarget) return

    const button = this.buttonTarget
    const original = button.dataset.originalText || button.textContent
    button.dataset.originalText = original
    button.textContent = this.successTextValue
    clearTimeout(this.resetTimer)
    this.resetTimer = setTimeout(() => {
      button.textContent = original
    }, 1500)
  }
}

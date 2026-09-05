import { Controller } from "@hotwired/stimulus"

// A button anywhere on the page that hands a question to the global Scout overlay.
export default class extends Controller {
  static values = { text: String }

  fire(event) {
    event.preventDefault()
    window.dispatchEvent(new CustomEvent("scout:ask", { detail: { text: this.textValue } }))
  }
}

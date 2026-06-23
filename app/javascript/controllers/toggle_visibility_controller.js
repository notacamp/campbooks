import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["content", "toggle"]

  toggle() {
    this.contentTarget.classList.toggle("hidden")
    this.contentTarget.classList.toggle("flex")
    this.toggleTarget.classList.toggle("hidden")
  }
}

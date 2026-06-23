import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  connect() {
    this.details = this.element
    this.boundClickOutside = this.clickOutside.bind(this)
    this.boundKeydown = this.keydown.bind(this)
    document.addEventListener("click", this.boundClickOutside)
    document.addEventListener("keydown", this.boundKeydown)
  }

  disconnect() {
    document.removeEventListener("click", this.boundClickOutside)
    document.removeEventListener("keydown", this.boundKeydown)
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this.details.open = false
    }
  }

  keydown(event) {
    if (event.key === "Escape" && this.details.open) {
      this.details.open = false
    }
  }
}

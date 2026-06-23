import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["panel"]

  connect() {
    this.boundClickOutside = this.clickOutside.bind(this)
    this.boundKeydown = this.keydown.bind(this)
    document.addEventListener("click", this.boundClickOutside)
    document.addEventListener("keydown", this.boundKeydown)
  }

  disconnect() {
    document.removeEventListener("click", this.boundClickOutside)
    document.removeEventListener("keydown", this.boundKeydown)
  }

  toggle(event) {
    event.stopPropagation()
    this.panelTarget.classList.toggle("hidden")
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this.panelTarget.classList.add("hidden")
    }
  }

  keydown(event) {
    if (event.key === "Escape" && !this.panelTarget.classList.contains("hidden")) {
      this.panelTarget.classList.add("hidden")
    }
  }
}

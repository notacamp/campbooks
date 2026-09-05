import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["panel", "trigger", "scrim"]

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
    if (this.panelTarget.classList.contains("hidden")) {
      this._open()
    } else {
      this._close()
    }
  }

  close() {
    this._close()
  }

  _open() {
    this.panelTarget.classList.remove("hidden")
    if (this.hasScrimTarget) this.scrimTarget.classList.remove("hidden")
    if (this.hasTriggerTarget) this.triggerTarget.setAttribute("aria-expanded", "true")
  }

  _close() {
    this.panelTarget.classList.add("hidden")
    if (this.hasScrimTarget) this.scrimTarget.classList.add("hidden")
    if (this.hasTriggerTarget) this.triggerTarget.setAttribute("aria-expanded", "false")
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this._close()
    }
  }

  keydown(event) {
    if (event.key === "Escape" && !this.panelTarget.classList.contains("hidden")) {
      this._close()
    }
  }
}

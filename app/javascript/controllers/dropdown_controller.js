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
    this._isOpen() ? this._close() : this._open()
  }

  close() {
    this._close()
  }

  // A <dialog> panel (the phone sheet) closes when its backdrop is tapped: the
  // click's target is the dialog element itself, never one of its children.
  backdropClose(event) {
    if (event.target === this.panelTarget) this._close()
  }

  _isDialog() {
    return this.panelTarget.tagName === "DIALOG"
  }

  _isOpen() {
    return this._isDialog() ? this.panelTarget.open : !this.panelTarget.classList.contains("hidden")
  }

  _open() {
    if (this._isDialog()) {
      if (!this.panelTarget.open) this.panelTarget.showModal()
    } else {
      this.panelTarget.classList.remove("hidden")
    }
    if (this.hasScrimTarget) this.scrimTarget.classList.remove("hidden")
    if (this.hasTriggerTarget) this.triggerTarget.setAttribute("aria-expanded", "true")
  }

  _close() {
    if (this._isDialog()) {
      if (this.panelTarget.open) this.panelTarget.close()
    } else {
      this.panelTarget.classList.add("hidden")
    }
    if (this.hasScrimTarget) this.scrimTarget.classList.add("hidden")
    if (this.hasTriggerTarget) this.triggerTarget.setAttribute("aria-expanded", "false")
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this._close()
    }
  }

  keydown(event) {
    if (event.key === "Escape" && this._isOpen()) {
      this._close()
    }
  }
}

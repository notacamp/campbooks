import { Controller } from "@hotwired/stimulus"

// Drives the mobile navigation panel in the topbar. The panel slides down
// under the topbar on small screens; closes on outside click, Escape, or
// Turbo navigation. Mirrors the dropdown/email-drawer controller conventions.
export default class extends Controller {
  static targets = ["panel", "backdrop", "openIcon", "closeIcon"]

  connect() {
    this.boundKeydown = this._keydown.bind(this)
    this.boundClose = this.close.bind(this)
    document.addEventListener("keydown", this.boundKeydown)
    document.addEventListener("turbo:before-visit", this.boundClose)
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKeydown)
    document.removeEventListener("turbo:before-visit", this.boundClose)
    this._unlockScroll()
  }

  toggle(event) {
    event?.stopPropagation()
    this.isOpen ? this.close() : this.open()
  }

  open() {
    this.isOpen = true
    this.panelTarget.classList.remove("hidden")
    this.backdropTarget?.classList.remove("hidden")
    this.openIconTarget?.classList.add("hidden")
    this.closeIconTarget?.classList.remove("hidden")
    this.element.querySelector("[aria-controls]")?.setAttribute("aria-expanded", "true")
    this._lockScroll()
  }

  close() {
    if (!this.isOpen) return
    this.isOpen = false
    this.panelTarget.classList.add("hidden")
    this.backdropTarget?.classList.add("hidden")
    this.openIconTarget?.classList.remove("hidden")
    this.closeIconTarget?.classList.add("hidden")
    this.element.querySelector("[aria-controls]")?.setAttribute("aria-expanded", "false")
    this._unlockScroll()
  }

  _keydown(event) {
    if (event.key === "Escape") this.close()
  }

  _lockScroll() {
    document.body.style.overflow = "hidden"
  }

  _unlockScroll() {
    document.body.style.overflow = ""
  }
}

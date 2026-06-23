import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["iframe", "overlay"]

  open() {
    this.overlayTarget.classList.remove("hidden")
    document.body.classList.add("overflow-hidden")
    this.overlayTarget.querySelector("iframe").src = this.iframeTarget.src
  }

  close() {
    this.overlayTarget.classList.add("hidden")
    document.body.classList.remove("overflow-hidden")
    this.overlayTarget.querySelector("iframe").src = ""
  }

  closeOnEscape(event) {
    if (event.key === "Escape") this.close()
  }

  closeOnBackdrop(event) {
    if (event.target === this.overlayTarget) this.close()
  }
}

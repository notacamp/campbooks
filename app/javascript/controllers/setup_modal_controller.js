import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["dialog", "frame"]

  connect() {
    this.dialogTarget.addEventListener("click", (e) => {
      if (e.target === this.dialogTarget) this.close()
    })
    this.dialogTarget.addEventListener("close", () => {
      this.frameTarget.src = ""
    })

    this._handleClick = (e) => {
      const trigger = e.target.closest("[data-setup-modal-open]")
      if (trigger) {
        e.preventDefault()
        this.frameTarget.src = trigger.getAttribute("data-setup-modal-open")
        this.dialogTarget.showModal()
      }
      const closer = e.target.closest("[data-setup-modal-close]")
      if (closer) {
        e.preventDefault()
        this.close()
      }
    }
    document.addEventListener("click", this._handleClick)
  }

  disconnect() {
    document.removeEventListener("click", this._handleClick)
  }

  close() {
    this.dialogTarget.close()
  }
}

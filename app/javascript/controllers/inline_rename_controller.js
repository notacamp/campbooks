import { Controller } from "@hotwired/stimulus"

// Tiny inline rename UX: a display span + pencil button.
// Clicking the pencil swaps the display for a text field. Submit on Enter/blur,
// Escape cancels and restores the original display. The form posts via turbo-stream.

export default class extends Controller {
  static targets = ["display", "form", "input", "editBtn"]

  activate () {
    this.displayTarget.classList.add("hidden")
    this.editBtnTarget.classList.add("hidden")
    this.formTarget.classList.remove("hidden")
    this.formTarget.classList.add("flex")
    this.inputTarget.value = this.displayTarget.textContent.trim()
    this.inputTarget.focus()
    this.inputTarget.select()
  }

  cancel () {
    this.formTarget.classList.add("hidden")
    this.formTarget.classList.remove("flex")
    this.displayTarget.classList.remove("hidden")
    this.editBtnTarget.classList.remove("hidden")
  }

  submit () {
    // The form submits via Turbo and re-renders the frame, which will reset state.
    // We don't need to do anything extra here.
  }
}

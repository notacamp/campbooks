import { Controller } from "@hotwired/stimulus"

// Opens/closes the New Folder <dialog> (Campbooks::NewFolderModal) and resets it
// after a successful create. The create response (a Turbo Stream) appends the new
// chip + toast; on a validation error it fills #new_folder_error and we stay open.
export default class extends Controller {
  static targets = ["dialog", "form", "input"]

  open() {
    if (!this.hasDialogTarget) return
    this.dialogTarget.showModal()
    if (this.hasInputTarget) this.inputTarget.focus()
  }

  close() {
    if (this.hasDialogTarget) this.dialogTarget.close()
  }

  // Native <dialog> backdrop click: the click target is the dialog element itself.
  backdropClose(e) {
    if (e.target === this.dialogTarget) this.close()
  }

  submitEnd(e) {
    if (!e.detail.success) return
    if (this.hasFormTarget) this.formTarget.reset()
    this.clearError()
    this.close()
  }

  clearError() {
    const err = document.getElementById("new_folder_error")
    if (err) err.innerHTML = ""
  }
}

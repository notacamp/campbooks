import { Controller } from "@hotwired/stimulus"

// Wires the interactive Drive folder picker into the config form.
// "Select this folder" buttons dispatch the select action, which reads
// the folder id and path from param attributes and populates the form fields.
export default class extends Controller {
  static targets = ["folderIdField", "folderPathField", "selectedLabel", "picker"]

  select(event) {
    const { params } = event
    const folderId = params.id
    const folderPath = params.path

    if (this.hasFolderIdFieldTarget) {
      this.folderIdFieldTarget.value = folderId === "root" ? "" : folderId
    }
    if (this.hasFolderPathFieldTarget) {
      this.folderPathFieldTarget.value = folderPath
    }
    if (this.hasSelectedLabelTarget) {
      this.selectedLabelTarget.textContent = folderPath
    }
    if (this.hasPickerTarget) {
      this.pickerTarget.innerHTML = ""
    }
  }
}

import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["dropzone", "input", "fileList", "fileListItems", "submit", "container"]

  toggle() {
    this.containerTarget.classList.toggle("hidden")
  }

  dragover(event) {
    event.preventDefault()
    this.dropzoneTarget.classList.add("border-indigo-500", "bg-indigo-50")
  }

  dragleave(event) {
    event.preventDefault()
    this.dropzoneTarget.classList.remove("border-indigo-500", "bg-indigo-50")
  }

  drop(event) {
    event.preventDefault()
    this.dropzoneTarget.classList.remove("border-indigo-500", "bg-indigo-50")

    const files = event.dataTransfer.files
    this.inputTarget.files = files
    this.displayFiles(files)
  }

  handleFiles() {
    this.displayFiles(this.inputTarget.files)
  }

  displayFiles(files) {
    if (files.length === 0) {
      this.fileListTarget.classList.add("hidden")
      return
    }

    this.fileListTarget.classList.remove("hidden")
    this.fileListItemsTarget.innerHTML = ""

    Array.from(files).forEach(file => {
      const li = document.createElement("li")
      li.className = "flex items-center justify-between p-2 bg-gray-50 rounded"
      li.innerHTML = `
        <span class="text-sm text-gray-700">${file.name}</span>
        <span class="text-xs text-gray-500">${(file.size / 1024).toFixed(1)} KB</span>
      `
      this.fileListItemsTarget.appendChild(li)
    })
  }
}

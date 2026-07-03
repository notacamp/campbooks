import { Controller } from "@hotwired/stimulus"

// Drives the composer's attachment tray. On file pick it uploads each file to
// the upload endpoint, shows a chip while uploading, and on success drops a
// hidden `attachments[]` signed-id input into the chip so the compose form
// submits the attachment set. Removing a chip removes its hidden input.
export default class extends Controller {
  static targets = ["fileInput", "tray"]
  static values = {
    uploadUrl: String,
    fieldName: { type: String, default: "attachments[]" },
    errorText: { type: String, default: "Upload failed" }
  }

  pick(event) {
    event?.preventDefault()
    this.fileInputTarget.click()
  }

  async upload(event) {
    const files = Array.from(event.target.files || [])
    event.target.value = ""
    for (const file of files) await this._uploadOne(file)
  }

  async _uploadOne(file) {
    const chip = this._addChip(file.name, file.size)
    const body = new FormData()
    body.append("file", file)
    try {
      const res = await fetch(this.uploadUrlValue, {
        method: "POST",
        body,
        headers: { "X-CSRF-Token": this._csrf(), "Accept": "application/json" }
      })
      const data = await res.json().catch(() => ({}))
      if (!res.ok || !data.signed_id) return this._failChip(chip, data.error)
      this._finalizeChip(chip, data.signed_id)
    } catch (_e) {
      this._failChip(chip)
    }
  }

  _addChip(name, size) {
    const chip = document.createElement("span")
    chip.className = "attachment-chip is-uploading"
    chip.dataset.filename = name
    if (size) chip.dataset.byteSize = size

    const label = document.createElement("span")
    label.className = "attachment-chip-name"
    label.textContent = size ? `${name} · ${this._humanSize(size)}` : name

    const remove = document.createElement("button")
    remove.type = "button"
    remove.setAttribute("aria-label", "Remove")
    remove.textContent = "✕"
    remove.addEventListener("click", () => {
      chip.remove()
      this._announce()
    })

    chip.append(label, remove)
    this.trayTarget.appendChild(chip)
    return chip
  }

  _finalizeChip(chip, signedId) {
    chip.classList.remove("is-uploading")
    const input = document.createElement("input")
    input.type = "hidden"
    input.name = this.fieldNameValue
    input.value = signedId
    chip.appendChild(input)
    this._announce()
  }

  // Chips change the submitted attachment set without any native form event —
  // announce it so the draft autosave picks the change up.
  _announce() {
    this.element.dispatchEvent(new Event("input", { bubbles: true }))
  }

  _failChip(chip, message) {
    chip.classList.remove("is-uploading")
    chip.classList.add("is-error")
    const label = chip.querySelector(".attachment-chip-name")
    if (label) label.textContent = message || this.errorTextValue
    setTimeout(() => chip.remove(), 4000)
  }

  _humanSize(bytes) {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
  }

  _csrf() {
    return document.querySelector("meta[name='csrf-token']")?.content || ""
  }
}

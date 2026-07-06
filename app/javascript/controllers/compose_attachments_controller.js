import { Controller } from "@hotwired/stimulus"

// Drives the composer's attachment tray. On file pick (or drag-and-drop) it
// uploads each file to the upload endpoint, shows a chip while uploading, and
// on success drops a hidden `attachments[]` signed-id input into the chip so
// the compose form submits the attachment set. Removing a chip removes its
// hidden input.
//
// When the tray lives outside the <form> element (context rail / card variant),
// pass `data-compose-attachments-form-id-value="<form-id>"` — the controller
// adds `form="<form-id>"` to every signed-id input so the browser still
// associates them with the correct form (HTML5 form-association).
export default class extends Controller {
  static targets = ["fileInput", "tray", "dropHint"]
  static values = {
    uploadUrl: String,
    fieldName: { type: String, default: "attachments[]" },
    errorText: { type: String, default: "Upload failed" },
    formId: { type: String, default: "" }
  }

  connect() {
    this._onDragOver = this._handleDragOver.bind(this)
    this._onDragLeave = this._handleDragLeave.bind(this)
    this._onDrop = this._handleDrop.bind(this)

    this.element.addEventListener("dragover", this._onDragOver)
    this.element.addEventListener("dragleave", this._onDragLeave)
    this.element.addEventListener("drop", this._onDrop)

    this._syncDropHint()
  }

  disconnect() {
    this.element.removeEventListener("dragover", this._onDragOver)
    this.element.removeEventListener("dragleave", this._onDragLeave)
    this.element.removeEventListener("drop", this._onDrop)
  }

  pick(event) {
    event?.preventDefault()
    this.fileInputTarget.click()
  }

  // Server-seeded chips (restored drafts, forwarded originals) bind their
  // remove button here instead of the addEventListener path used for uploads.
  removeChip(event) {
    event.currentTarget.closest(".attachment-chip")?.remove()
    this._syncDropHint()
    this._announce()
  }

  async upload(event) {
    const files = Array.from(event.target.files || [])
    event.target.value = ""
    for (const file of files) await this._uploadOne(file)
  }

  // ── Drag-and-drop ─────────────────────────────────────────

  _handleDragOver(event) {
    if (!Array.from(event.dataTransfer.types).includes("Files")) return
    event.preventDefault()
    this.element.classList.add("is-drag-over")
  }

  _handleDragLeave(event) {
    if (this.element.contains(event.relatedTarget)) return
    this.element.classList.remove("is-drag-over")
  }

  _handleDrop(event) {
    event.preventDefault()
    this.element.classList.remove("is-drag-over")
    const files = Array.from(event.dataTransfer.files || [])
    files.forEach(file => this._uploadOne(file))
  }

  // ── Upload pipeline ───────────────────────────────────────

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
      this._syncDropHint()
      this._announce()
    })

    chip.append(label, remove)
    this.trayTarget.appendChild(chip)
    this._syncDropHint()
    return chip
  }

  _finalizeChip(chip, signedId) {
    chip.classList.remove("is-uploading")
    const input = document.createElement("input")
    input.type = "hidden"
    input.name = this.fieldNameValue
    input.value = signedId
    // Associate with an out-of-form tray via HTML5 form attribute.
    if (this.formIdValue) input.setAttribute("form", this.formIdValue)
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

  // Show the drop-hint placeholder only when no chips are present.
  _syncDropHint() {
    if (!this.hasDropHintTarget) return
    const hasChips = this.trayTarget.querySelector(".attachment-chip") != null
    this.dropHintTarget.style.display = hasChips ? "none" : ""
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

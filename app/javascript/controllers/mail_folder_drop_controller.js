import { Controller } from "@hotwired/stimulus"

// Move emails into a folder from the inbox folder bar:
//   • Desktop — drag a row's [data-drag-handle] onto a folder chip.
//   • Mobile / no-drag — select rows (checkboxes), then tap a chip.
// Both POST a move_to_folder to the bulk endpoint and render the Turbo Stream it
// returns (rows removed + an Undo toast). When nothing is selected, a chip click
// falls through to its normal folder-filter navigation.
export default class extends Controller {
  static targets = ["chip"]

  connect() {
    this._over = null
  }

  disconnect() {
    this._unhighlight()
  }

  // --- Drag and drop (desktop) ---

  dragover(e) {
    const chip = this._chipFrom(e)
    if (!chip) return
    e.preventDefault() // mark this chip as a valid drop target
    e.dataTransfer.dropEffect = "move"
    if (this._over !== chip) { this._unhighlight(); this._highlight(chip) }
  }

  dragleave(e) {
    const chip = this._chipFrom(e)
    if (chip && !chip.contains(e.relatedTarget)) this._clear(chip)
  }

  drop(e) {
    const chip = this._chipFrom(e)
    if (!chip) return
    e.preventDefault()
    this._unhighlight()
    const ids = (e.dataTransfer.getData("text/plain") || "")
      .split(",").map((s) => s.trim()).filter(Boolean)
    if (ids.length) this._move(ids, chip)
  }

  // --- Tap to move selected (mobile / no-drag) ---

  click(e) {
    const chip = this._chipFrom(e)
    if (!chip) return
    const ids = this._selectedIds()
    if (ids.length === 0) return // no selection → let the chip navigate (filter)
    e.preventDefault()
    e.stopPropagation()
    this._move(ids, chip)
    this._clearSelection()
  }

  // --- Shared ---

  _move(ids, chip) {
    const body = new URLSearchParams()
    body.append("tool", "move_to_folder")
    body.append("folder_name", chip.dataset.folderName || "")
    const from = this._activeFolderName()
    if (from) body.append("from", from)
    ids.forEach((id) => body.append("email_ids[]", id))

    fetch("/email_messages/bulk", {
      method: "POST",
      headers: {
        "X-CSRF-Token": this._csrf(),
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: body.toString()
    })
      .then((r) => (r.ok ? r.text() : Promise.reject(r)))
      .then((html) => { if (window.Turbo && html) window.Turbo.renderStreamMessage(html) })
      .catch(() => {})
  }

  _chipFrom(e) {
    return e.target.closest('[data-mail-folder-drop-target="chip"]')
  }

  _activeFolderName() {
    const active = this.chipTargets.find((c) => c.dataset.folderActive === "true")
    return active ? active.dataset.folderName : ""
  }

  _selectedIds() {
    return Array.from(document.querySelectorAll('[data-email-selection-target="checkbox"]:checked')).map((c) => c.value)
  }

  _clearSelection() {
    document.querySelectorAll('[data-email-selection-target="checkbox"]:checked').forEach((c) => {
      c.checked = false
      c.dispatchEvent(new Event("change", { bubbles: true }))
    })
  }

  _highlight(chip) { this._over = chip; chip.classList.add("ring-2", "ring-accent-500", "ring-inset") }
  _clear(chip) { chip.classList.remove("ring-2", "ring-accent-500", "ring-inset"); if (this._over === chip) this._over = null }
  _unhighlight() { if (this._over) this._clear(this._over) }

  _csrf() { return document.querySelector('meta[name="csrf-token"]')?.content }
}

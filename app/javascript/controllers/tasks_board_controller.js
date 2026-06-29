import { Controller } from "@hotwired/stimulus"

// Drag-and-drop for the task status board. Native HTML5 DnD (no library), modeled
// on inbox_board_controller. Dropping a card in another column moves it
// optimistically in the DOM and PATCHes { status } to that task's move_url; on
// failure the card snaps back to its origin column.
export default class extends Controller {
  static targets = ["card", "dropzone"]

  connect() {
    this.dragged = null
    this.origin = null
    this.cardTargets.forEach((card) => {
      card.addEventListener("dragstart", (e) => this.onDragStart(e))
      card.addEventListener("dragend", () => this.onDragEnd())
    })
    this.dropzoneTargets.forEach((zone) => {
      zone.addEventListener("dragover", (e) => this.onDragOver(e, zone))
      zone.addEventListener("dragleave", () => this.clearHighlight(zone))
      zone.addEventListener("drop", (e) => this.onDrop(e, zone))
    })
  }

  onDragStart(event) {
    const card = event.currentTarget
    this.dragged = card
    this.origin = card.closest("[data-tasks-board-target='dropzone']")
    card.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
    try { event.dataTransfer.setData("text/plain", card.dataset.taskId || "") } catch (_e) { /* noop */ }
  }

  onDragEnd() {
    if (this.dragged) this.dragged.classList.remove("opacity-50")
    this.dropzoneTargets.forEach((zone) => this.clearHighlight(zone))
    this.dragged = null
    this.origin = null
  }

  onDragOver(event, zone) {
    if (!this.dragged || zone === this.origin) return
    event.preventDefault()
    event.dataTransfer.dropEffect = "move"
    zone.classList.add("ring-2", "ring-accent-400", "ring-inset")
  }

  clearHighlight(zone) {
    zone.classList.remove("ring-2", "ring-accent-400", "ring-inset")
  }

  onDrop(event, zone) {
    event.preventDefault()
    this.clearHighlight(zone)

    const card = this.dragged
    const origin = this.origin
    if (!card || zone === origin) return

    const to = zone.dataset.column

    // Optimistic move, then confirm with the server.
    zone.appendChild(card)
    this._move(card.dataset.moveUrl, to)
      .then((ok) => { if (!ok) this._revert(card, origin) })
      .catch(() => this._revert(card, origin))
  }

  _revert(card, origin) {
    if (origin) origin.appendChild(card)
    card.classList.add("ring-2", "ring-red-400")
    setTimeout(() => card.classList.remove("ring-2", "ring-red-400"), 1200)
  }

  _move(url, status) {
    return fetch(url, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": this.csrf,
        Accept: "application/json"
      },
      body: JSON.stringify({ status })
    }).then((r) => r.ok)
  }

  get csrf() {
    return document.querySelector('meta[name="csrf-token"]')?.content || ""
  }
}

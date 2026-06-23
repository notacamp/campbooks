import { Controller } from "@hotwired/stimulus"

// Drag-and-drop for the inbox status board. Native HTML5 DnD (no library),
// modeled on folder_sort_controller. Dragging a card to another column moves it
// optimistically in the DOM and POSTs { thread_id, from, to } to board_move; on
// failure the card snaps back to its origin. The read-only Awaiting column
// (data-droppable="false") rejects drops. Column counts refresh on next open.
export default class extends Controller {
  static targets = ["card", "dropzone"]
  static values = { moveUrl: String }

  connect() {
    this.dragged = null
    this.origin = null
    this._boundDragStart = this.onDragStart.bind(this)
    this._boundDragEnd = this.onDragEnd.bind(this)
    this.cardTargets.forEach((card) => {
      card.addEventListener("dragstart", this._boundDragStart)
      card.addEventListener("dragend", this._boundDragEnd)
    })
    this.dropzoneTargets.forEach((zone) => {
      zone.addEventListener("dragover", (e) => this.onDragOver(e, zone))
      zone.addEventListener("dragleave", () => this.clearHighlight(zone))
      zone.addEventListener("drop", (e) => this.onDrop(e, zone))
    })
  }

  onDragStart(event) {
    const card = event.currentTarget
    if (card.getAttribute("draggable") !== "true") {
      event.preventDefault()
      return
    }
    this.dragged = card
    this.origin = card.closest("[data-inbox-board-target='dropzone']")
    card.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
    // Some browsers require data to be set for the drag to begin.
    try { event.dataTransfer.setData("text/plain", card.dataset.threadId || "") } catch (_e) { /* noop */ }
  }

  onDragEnd() {
    if (this.dragged) this.dragged.classList.remove("opacity-50")
    this.dropzoneTargets.forEach((zone) => this.clearHighlight(zone))
    this.dragged = null
    this.origin = null
  }

  onDragOver(event, zone) {
    if (!this.dragged) return
    if (zone.dataset.droppable !== "true") return // read-only column
    if (zone === this.origin) return
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
    if (!card || zone.dataset.droppable !== "true" || zone === origin) return

    const from = origin && origin.dataset.column
    const to = zone.dataset.column
    const threadId = card.dataset.threadId

    // Optimistic move, then confirm with the server.
    zone.appendChild(card)
    card.dataset.column = to

    this._post(threadId, from, to)
      .then((ok) => { if (!ok) this._revert(card, origin) })
      .catch(() => this._revert(card, origin))
  }

  _revert(card, origin) {
    if (origin) origin.appendChild(card)
    card.classList.add("ring-2", "ring-red-400")
    setTimeout(() => card.classList.remove("ring-2", "ring-red-400"), 1200)
  }

  _post(threadId, from, to) {
    return fetch(this.moveUrlValue, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": this.csrf,
        Accept: "application/json"
      },
      body: JSON.stringify({ thread_id: threadId, from, to })
    }).then((r) => r.ok)
  }

  get csrf() {
    return document.querySelector('meta[name="csrf-token"]')?.content || ""
  }
}

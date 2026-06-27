import { Controller } from "@hotwired/stimulus"

// Drag-and-drop for the pipeline kanban board. Direct adaptation of
// inbox_board_controller: drag a card to another stage column,
// POST { membership_id, to_stage_id } to the move endpoint, revert on failure.
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

  cardTargetConnected() {
    const card = this.cardTargets[this.cardTargets.length - 1]
    if (card) {
      card.addEventListener("dragstart", this._boundDragStart)
      card.addEventListener("dragend", this._boundDragEnd)
    }
  }

  onDragStart(event) {
    const card = event.currentTarget
    if (card.getAttribute("draggable") !== "true") {
      event.preventDefault()
      return
    }
    this.dragged = card
    this.origin = card.closest("[data-pipeline-board-target='dropzone']")
    card.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
    try { event.dataTransfer.setData("text/plain", card.dataset.membershipId || "") } catch (_e) {}
  }

  onDragEnd() {
    if (this.dragged) this.dragged.classList.remove("opacity-50")
    this.dropzoneTargets.forEach((z) => this.clearHighlight(z))
    this.dragged = null
    this.origin = null
  }

  onDragOver(event, zone) {
    if (!this.dragged) return
    if (zone.dataset.droppable !== "true") return
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

    const membershipId = card.dataset.membershipId
    const toStageId = zone.dataset.stageId

    // Optimistic move
    zone.appendChild(card)
    card.dataset.stageId = toStageId

    this._post(membershipId, toStageId)
      .then((ok) => { if (!ok) this._revert(card, origin) })
      .catch(() => this._revert(card, origin))
  }

  _revert(card, origin) {
    if (origin) origin.appendChild(card)
    card.classList.add("ring-2", "ring-red-400")
    setTimeout(() => card.classList.remove("ring-2", "ring-red-400"), 1200)
  }

  _post(membershipId, toStageId) {
    return fetch(this.moveUrlValue, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": this.csrf,
        Accept: "application/json"
      },
      body: JSON.stringify({ membership_id: membershipId, to_stage_id: toStageId })
    }).then((r) => r.ok)
  }

  get csrf() {
    return document.querySelector('meta[name="csrf-token"]')?.content || ""
  }
}

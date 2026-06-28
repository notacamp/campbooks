import { Controller } from "@hotwired/stimulus"

// Drag-and-drop for the pipeline kanban board. Drag a card to another stage
// column, optimistically move it, POST { membership_id, to_stage_id } to the
// move endpoint, and revert on failure. Card/dropzone listeners are wired
// declaratively via data-action in the view, so dynamically-added cards (from
// the item picker) just work — no manual (re)binding, no double listeners.
export default class extends Controller {
  static targets = ["dropzone"]
  static values = { moveUrl: String }

  connect() {
    this.dragged = null
    this.origin = null
  }

  dragStart(event) {
    const card = event.currentTarget
    if (card.getAttribute("draggable") !== "true") {
      event.preventDefault()
      return
    }
    this.dragged = card
    this.origin = card.closest("[data-pipeline-board-target='dropzone']")
    card.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
    try { event.dataTransfer.setData("text/plain", card.dataset.membershipId || "") } catch (_e) { /* Safari */ }
  }

  dragEnd() {
    if (this.dragged) this.dragged.classList.remove("opacity-50")
    this.dropzoneTargets.forEach((z) => this.clearHighlight(z))
    this.dragged = null
    this.origin = null
  }

  dragOver(event) {
    const zone = event.currentTarget
    if (!this.dragged || zone.dataset.droppable !== "true" || zone === this.origin) return
    event.preventDefault()
    event.dataTransfer.dropEffect = "move"
    zone.classList.add("ring-2", "ring-accent-400", "ring-inset")
  }

  dragLeave(event) {
    const zone = event.currentTarget
    if (zone.contains(event.relatedTarget)) return // moved onto a child, not actually leaving
    this.clearHighlight(zone)
  }

  clearHighlight(zone) {
    zone.classList.remove("ring-2", "ring-accent-400", "ring-inset")
  }

  drop(event) {
    const zone = event.currentTarget
    event.preventDefault()
    this.clearHighlight(zone)

    const card = this.dragged
    const origin = this.origin
    if (!card || zone.dataset.droppable !== "true" || zone === origin) return

    const membershipId = card.dataset.membershipId
    const toStageId = zone.dataset.stageId

    // Optimistic move; revert if the server rejects it.
    zone.appendChild(card)
    card.dataset.stageId = toStageId

    this.post(membershipId, toStageId)
      .then((ok) => { if (!ok) this.revert(card, origin) })
      .catch(() => this.revert(card, origin))
  }

  revert(card, origin) {
    if (origin) origin.appendChild(card)
    card.classList.add("ring-2", "ring-red-400")
    setTimeout(() => card.classList.remove("ring-2", "ring-red-400"), 1200)
  }

  post(membershipId, toStageId) {
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

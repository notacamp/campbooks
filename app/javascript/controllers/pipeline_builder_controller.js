import { Controller } from "@hotwired/stimulus"

// Manages inline stage editing within the pipeline form: add, remove, and
// reorder stages. Each stage row is a nested fields_for rendered inside a
// data-pipeline-builder-target="stages" container.
export default class extends Controller {
  static targets = ["stages", "stageRow", "stageTemplate", "position"]

  connect() {
    this._stageIndex = this.stageRowTargets.length
  }

  addStage() {
    if (!this.hasStageTemplateTarget) return
    const content = this.stageTemplateTarget.innerHTML.replaceAll(
      "NEW_RECORD", this._stageIndex.toString()
    )
    this.stagesTarget.insertAdjacentHTML("beforeend", content)
    this._stageIndex++
  }

  removeStage(event) {
    const row = event.currentTarget.closest("[data-pipeline-builder-target='stageRow']")
    if (!row) return

    const destroyInput = row.querySelector("input[name*='[_destroy]']")
    if (destroyInput) {
      destroyInput.value = "1"
      row.classList.add("hidden")
    } else {
      row.remove()
    }
  }

  // Drag-to-reorder stages within the pipeline form using native HTML5 DnD.
  dragStart(event) {
    this._draggedRow = event.currentTarget.closest("[data-pipeline-builder-target='stageRow']")
    if (!this._draggedRow) return
    this._draggedRow.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
  }

  dragOver(event) {
    event.preventDefault()
    const row = event.currentTarget.closest("[data-pipeline-builder-target='stageRow']")
    if (!row || row === this._draggedRow) return
    event.dataTransfer.dropEffect = "move"
    row.classList.add("border-accent-400")
  }

  dragLeave(event) {
    const row = event.currentTarget.closest("[data-pipeline-builder-target='stageRow']")
    if (row) row.classList.remove("border-accent-400")
  }

  drop(event) {
    event.preventDefault()
    const dropRow = event.currentTarget.closest("[data-pipeline-builder-target='stageRow']")
    if (!dropRow || dropRow === this._draggedRow) return

    dropRow.classList.remove("border-accent-400")

    // Insert before or after the drop target based on cursor position
    const rect = dropRow.getBoundingClientRect()
    const mid = rect.top + rect.height / 2
    if (event.clientY < mid) {
      dropRow.before(this._draggedRow)
    } else {
      dropRow.after(this._draggedRow)
    }

    this._draggedRow.classList.remove("opacity-50")
    this._renumberPositions()
    this._draggedRow = null
  }

  dragEnd() {
    if (this._draggedRow) {
      this._draggedRow.classList.remove("opacity-50")
    }
    this.stageRowTargets.forEach((r) => r.classList.remove("border-accent-400"))
    this._draggedRow = null
  }

  _renumberPositions() {
    const rows = this.stagesTarget.querySelectorAll("[data-pipeline-builder-target='stageRow']:not(.hidden)")
    rows.forEach((row, i) => {
      const posInput = row.querySelector("[data-pipeline-builder-target='position']")
      if (posInput) posInput.value = i + 1
    })
  }
}

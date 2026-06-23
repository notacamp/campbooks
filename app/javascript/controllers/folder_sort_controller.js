import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["item"]

  connect() {
    this.setupDragAndDrop()
  }

  setupDragAndDrop() {
    this.itemTargets.forEach(item => {
      item.draggable = true
      item.addEventListener("dragstart", this.onDragStart.bind(this))
      item.addEventListener("dragover", this.onDragOver.bind(this))
      item.addEventListener("drop", this.onDrop.bind(this))
      item.addEventListener("dragend", this.onDragEnd.bind(this))
    })
  }

  onDragStart(event) {
    this.draggedItem = event.currentTarget
    event.currentTarget.classList.add("opacity-50")
    event.dataTransfer.effectAllowed = "move"
  }

  onDragOver(event) {
    event.preventDefault()
    event.dataTransfer.dropEffect = "move"
  }

  onDrop(event) {
    event.preventDefault()
    const target = event.currentTarget
    if (target !== this.draggedItem) {
      const parent = target.parentNode
      const items = [...parent.querySelectorAll("[data-folder-sort-target='item']")]
      const fromIndex = items.indexOf(this.draggedItem)
      const toIndex = items.indexOf(target)

      if (fromIndex < toIndex) {
        parent.insertBefore(this.draggedItem, target.nextSibling)
      } else {
        parent.insertBefore(this.draggedItem, target)
      }

      this.saveOrder()
    }
  }

  onDragEnd(event) {
    event.currentTarget.classList.remove("opacity-50")
    this.draggedItem = null
  }

  saveOrder() {
    const items = this.element.querySelectorAll("[data-folder-sort-target='item']")
    const positions = [...items].map((item, index) => ({
      id: item.dataset.folderId,
      position: index
    }))

    // Find the email account ID from the page
    const accountId = this.element.dataset.emailAccountId

    fetch(`/email_accounts/${accountId}/email_folders/reorder`, {
      method: "PATCH",
      headers: {
        "Content-Type": "application/json",
        "X-CSRF-Token": document.querySelector("[name='csrf-token']").content
      },
      body: JSON.stringify({ positions })
    })
  }
}

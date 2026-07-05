import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["dropdown", "search", "results"]
  static values = {
    messageId: String,
    allTags: Array,
    assignedIds: Array
  }

  connect() {
    this.highlightedIndex = -1
    this.boundClickOutside = this.clickOutside.bind(this)
  }

  disconnect() {
    document.removeEventListener("click", this.boundClickOutside)
  }

  open() {
    this.dropdownTarget.classList.remove("hidden")
    this.searchTarget.value = ""
    this.searchTarget.focus()
    this.filterResults("")
    document.addEventListener("click", this.boundClickOutside)
  }

  close() {
    this.dropdownTarget.classList.add("hidden")
    this.searchTarget.value = ""
    this.highlightedIndex = -1
    document.removeEventListener("click", this.boundClickOutside)
  }

  clickOutside(event) {
    if (!this.element.contains(event.target)) {
      this.close()
    }
  }

  search() {
    this.filterResults(this.searchTarget.value.toLowerCase())
    this.highlightedIndex = -1
  }

  filterResults(query) {
    const available = this.allTagsValue.filter(
      tag => !this.assignedIdsValue.includes(tag.id) && tag.name.toLowerCase().includes(query)
    )

    if (available.length === 0) {
      this.resultsTarget.innerHTML = `<div class="px-3 py-2 text-[11px] text-gray-400">${query ? 'No matching tags' : 'All tags assigned'}</div>`
      return
    }

    this.resultsTarget.innerHTML = available.map((tag, index) => `
      <button type="button"
              data-action="click->email-tags#add mouseenter->email-tags#highlight"
              data-email-tags-tag-id-param="${tag.id}"
              data-email-tags-index-param="${index}"
              class="w-full text-left px-3 py-1.5 text-xs hover:bg-gray-100 flex items-center gap-2">
        <span class="w-2 h-2 rounded-full flex-shrink-0" style="background-color:${tag.color}"></span>
        <span>${this.escapeHtml(tag.name)}</span>
      </button>
    `).join("")
  }

  highlight(event) {
    const index = parseInt(event.currentTarget.dataset.emailTagsIndexParam)
    this.highlightedIndex = index
    const items = this.resultsTarget.querySelectorAll("button")
    items.forEach((btn, i) => btn.classList.toggle("bg-gray-100", i === index))
  }

  navigate(event) {
    const items = this.resultsTarget.querySelectorAll("button")
    if (items.length === 0) return

    if (event.key === "ArrowDown") {
      event.preventDefault()
      this.highlightedIndex = Math.min(this.highlightedIndex + 1, items.length - 1)
      this.applyHighlight(items)
    } else if (event.key === "ArrowUp") {
      event.preventDefault()
      this.highlightedIndex = Math.max(this.highlightedIndex - 1, 0)
      this.applyHighlight(items)
    } else if (event.key === "Enter") {
      event.preventDefault()
      if (this.highlightedIndex >= 0) {
        this.add({ currentTarget: items[this.highlightedIndex] })
      }
    } else if (event.key === "Escape") {
      this.close()
    }
  }

  applyHighlight(items) {
    items.forEach((btn, i) => btn.classList.toggle("bg-gray-100", i === this.highlightedIndex))
  }

  async add(event) {
    const tagId = event.currentTarget.dataset.emailTagsTagIdParam
    const csrfToken = document.querySelector('meta[name="csrf-token"]')?.content

    const response = await fetch(
      `/email_messages/${this.messageIdValue}/tags?tag_id=${tagId}`,
      {
        method: "POST",
        headers: {
          "Accept": "text/vnd.turbo-stream.html",
          "X-CSRF-Token": csrfToken
        }
      }
    )

    if (response.ok) {
      const html = await response.text()
      Turbo.renderStreamMessage(html)
    }
  }

  async remove(event) {
    event.stopPropagation()
    const tagId = event.currentTarget.dataset.emailTagsTagIdParam
    const csrfToken = document.querySelector('meta[name="csrf-token"]')?.content

    const response = await fetch(
      `/email_messages/${this.messageIdValue}/tags/${tagId}`,
      {
        method: "DELETE",
        headers: {
          "Accept": "text/vnd.turbo-stream.html",
          "X-CSRF-Token": csrfToken
        }
      }
    )

    if (response.ok) {
      const html = await response.text()
      Turbo.renderStreamMessage(html)
    }
  }

  escapeHtml(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }
}

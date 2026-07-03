import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["item"]

  connect() {
    // Restore selection from URL on page load. Message ids are UUIDs (uuid
    // primary keys); a `\d+`-only match truncated them, so the URL's row was
    // never re-selected on load — match a UUID (or numeric id) instead.
    const pathId = window.location.pathname.match(/\/email_messages\/([0-9a-f-]{8,}|\d+)/i)
    if (pathId) {
      const match = this.itemTargets.find(el => el.href.endsWith(`/${pathId[1]}`))
      if (match) {
        this.selectItem(match)
        return
      }
    }
    // Default: select first
    if (this.itemTargets.length > 0 && !this.hasSelectedItem()) {
      this.selectItem(this.itemTargets[0])
      const frame = document.getElementById("email_detail")
      if (frame && this.itemTargets[0].href) {
        frame.src = this.itemTargets[0].href
      }
    }
  }

  select(event) {
    event.preventDefault()
    const el = event.currentTarget
    this.selectItem(el)
    this.markRead(el)
    window.history.pushState({}, "", el.href)
    const frame = document.getElementById("email_detail")
    if (frame && el.href) {
      frame.src = el.href
    }
  }

  selectItem(el) {
    this.itemTargets.forEach(e => {
      e.classList.remove("bg-gray-100", "text-accent-700")
    })
    el.classList.add("bg-gray-100", "text-accent-700")
  }

  markRead(el) {
    // Remove unread dot
    const dot = el.querySelector(".bg-blue-500")
    if (dot) dot.remove()
    // Remove bold from subject
    const subject = el.querySelector(".font-bold")
    if (subject) {
      subject.classList.remove("font-bold")
      subject.style.fontWeight = "600"
    }
    // Reset date color
    const date = el.querySelector(".font-medium.text-gray-600")
    if (date) {
      date.classList.remove("font-medium", "text-gray-600")
      date.classList.add("text-gray-400")
    }
  }

  hasSelectedItem() {
    return this.itemTargets.some(el => el.classList.contains("bg-gray-100"))
  }
}

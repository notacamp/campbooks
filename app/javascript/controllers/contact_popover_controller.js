import { Controller } from "@hotwired/stimulus"

const CACHE = {}
const GRACE_MS = 150

export default class extends Controller {
  static values = { contactId: Number, email: String, url: String, profileUrl: String }

  connect() {
    this.enterTimeout = null
    this.leaveTimeout = null
    this.popover = null
  }

  disconnect() {
    this._clearTimers()
    this._removePopover()
  }

  // Touch / no-hover devices have no hover popover, so a tap on the avatar
  // navigates straight to the contact's profile — overriding whatever wraps the
  // avatar (e.g. an email-row link). Desktop (hover-capable) is left untouched:
  // the click falls through and the popover still appears on hover.
  click(event) {
    if (!this._isTouch() || !this.profileUrlValue) return
    event.preventDefault()
    event.stopPropagation()
    if (window.Turbo) {
      window.Turbo.visit(this.profileUrlValue)
    } else {
      window.location.href = this.profileUrlValue
    }
  }

  mouseEnter() {
    if (this._isTouch()) return
    this._clearTimers()
    this.enterTimeout = setTimeout(() => this._show(), 300)
  }

  _isTouch() {
    return window.matchMedia("(hover: none)").matches
  }

  mouseLeave() {
    this._clearTimers()
    this.leaveTimeout = setTimeout(() => this._removePopover(), GRACE_MS)
  }

  popoverEnter() {
    this._clearTimers()
  }

  popoverLeave() {
    this._removePopover()
  }

  _clearTimers() {
    clearTimeout(this.enterTimeout)
    clearTimeout(this.leaveTimeout)
  }

  _show() {
    const cacheKey = this.urlValue || this.contactIdValue || this.emailValue
    if (!cacheKey) return

    if (CACHE[cacheKey]) {
      this._renderPopover(CACHE[cacheKey])
    } else {
      this._fetchPopover(cacheKey)
    }
  }

  async _fetchPopover(cacheKey) {
    let url
    if (this.urlValue) {
      url = this.urlValue
    } else if (this.contactIdValue) {
      url = `/contacts/${this.contactIdValue}/popover`
    } else {
      url = `/contacts/popover?email=${encodeURIComponent(this.emailValue)}`
    }

    try {
      const response = await fetch(url, { headers: { "Accept": "text/html" } })
      if (!response.ok) return
      const html = await response.text()
      CACHE[cacheKey] = html
      this._renderPopover(html)
    } catch (_) {
      // Likely no contact found; silently ignore
    }
  }

  _renderPopover(html) {
    this._removePopover()

    this.popover = document.createElement("div")
    this.popover.innerHTML = html
    this.popover.className = "absolute z-[100]"
    this.popover.setAttribute("data-contact-popover", "true")
    this.popover.addEventListener("mouseenter", this.popoverEnter.bind(this))
    this.popover.addEventListener("mouseleave", this.popoverLeave.bind(this))
    // A star/block from inside the popover re-renders its actions in place via
    // Turbo Stream, but the cached HTML is now stale — drop the cache so the
    // next hover re-fetches the fresh state. Keep the popover open afterwards.
    this.popover.addEventListener("turbo:submit-end", () => {
      for (const key in CACHE) delete CACHE[key]
      this._clearTimers()
    })

    document.body.appendChild(this.popover)

    this._positionPopover()
  }

  _positionPopover() {
    if (!this.popover) return

    const trigger = this.element.getBoundingClientRect()
    const popoverEl = this.popover.firstElementChild
    if (!popoverEl) return

    // Position below the trigger, right-aligned
    const popoverRect = popoverEl.getBoundingClientRect()
    let left = trigger.left
    let top = trigger.bottom + 4

    // Keep popover within viewport
    if (left + popoverRect.width > window.innerWidth - 8) {
      left = window.innerWidth - popoverRect.width - 8
    }
    if (left < 8) left = 8
    if (top + popoverRect.height > window.innerHeight - 8) {
      top = trigger.top - popoverRect.height - 4
    }

    this.popover.style.left = `${left}px`
    this.popover.style.top = `${top}px`
  }

  _removePopover() {
    this._clearTimers()
    if (this.popover) {
      this.popover.remove()
      this.popover = null
    }
  }
}

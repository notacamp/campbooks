import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  static targets = ["search", "hidden", "dropdown", "pills"]
  static values = { url: String, inferred: Boolean }

  connect() {
    this.debounceTimer = null
    this.selectedIndex = -1
    this.loadInitialPills()
    this.setupClickOutside()
  }

  disconnect() {
    document.removeEventListener("click", this.clickOutsideHandler)
  }

  loadInitialPills() {
    const raw = this.hiddenTarget.value.trim()
    if (!raw) return
    // Server-seeded recipients are not user edits — don't announce them, or
    // opening a prefilled reply would immediately autosave a phantom draft.
    this._initializing = true
    raw.split(",").forEach(part => {
      const value = part.trim()
      if (!value) return
      if (this.inferredValue) {
        // Show the friendly name from a "Name <email>" value; keep the full value
        // as data-email so the sent address is unchanged. Mark it inferred.
        this.addPill(value, this._displayName(value), true)
      } else {
        this.addPill(value, value)
      }
    })
    this._initializing = false
  }

  // Parse "Sofia Martins <sofia@example.com>" → "Sofia Martins"; a bare address
  // stays as-is.
  _displayName(value) {
    const match = value.match(/^\s*(.+?)\s*<[^>]+>\s*$/)
    return match ? match[1].replace(/^"|"$/g, "") : value
  }

  // The "· inferred" suffix is a courtesy of the prefill — any edit makes the
  // recipients the user's own, so drop every suffix on the first change.
  _clearInferred() {
    if (!this.inferredValue) return
    this.inferredValue = false
    this.pillsTarget.querySelectorAll("[data-inferred-suffix]").forEach(el => el.remove())
  }

  // --- Search ---

  search() {
    clearTimeout(this.debounceTimer)
    this._clearInferred()
    const query = this.searchTarget.value.trim()

    if (query.length < 2) {
      this.hideDropdown()
      return
    }

    this.debounceTimer = setTimeout(() => {
      this.fetchResults(query)
    }, 200)
  }

  fetchResults(query) {
    const url = this.urlValue || "/contacts/search"
    fetch(`${url}?q=${encodeURIComponent(query)}`, {
      headers: { "Accept": "application/json" }
    })
      .then(r => r.json())
      .then(data => {
        this.renderDropdown(data)
      })
      .catch(() => this.hideDropdown())
  }

  renderDropdown(results) {
    if (!results || results.length === 0) {
      this.hideDropdown()
      return
    }

    this.selectedIndex = -1
    this.dropdownTarget.innerHTML = results.map((c, i) => {
      const name = c.display_name || c.name || c.email
      const label = c.name ? `${name} <span class="text-gray-400">${c.email}</span>` : name
      return `<button type="button" data-index="${i}" data-email="${this.escapeHtml(c.email)}" data-display="${this.escapeHtml(name)}" class="w-full text-left px-2.5 py-1.5 text-sm hover:bg-accent-50 flex items-center gap-2">
        <span class="w-6 h-6 rounded-full bg-gray-200 flex items-center justify-center text-[10px] font-medium text-gray-500 flex-shrink-0">${this.escapeHtml((name[0] || "?").toUpperCase())}</span>
        <span>${label}</span>
      </button>`
    }).join("")

    this.dropdownTarget.querySelectorAll("button").forEach(btn => {
      btn.addEventListener("mousedown", (e) => {
        e.preventDefault()
        this.selectResult(btn)
      })
    })

    this.dropdownTarget.classList.remove("hidden")
  }

  selectResult(btn) {
    this._clearInferred()
    const email = btn.dataset.email
    const display = btn.dataset.display || email
    this.addPill(email, display)
    this.searchTarget.value = ""
    this.hideDropdown()
  }

  hideDropdown() {
    this.dropdownTarget.classList.add("hidden")
    this.dropdownTarget.innerHTML = ""
    this.selectedIndex = -1
  }

  // --- Pills ---

  addPill(email, display, inferred = false) {
    // Dedup against the pills already rendered, not the hidden input. The hidden
    // input is server-seeded with the initial recipients, so checking it here
    // would make loadInitialPills() bail on every pre-filled address.
    const existing = Array.from(this.pillsTarget.querySelectorAll("[data-email]"))
      .map(pill => pill.dataset.email)
    if (existing.includes(email)) return

    const pill = document.createElement("span")
    pill.className = "inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-accent-100 text-accent-800 rounded-md border border-accent-200 whitespace-nowrap"
    pill.dataset.email = email
    const suffix = inferred
      ? `<span data-inferred-suffix class="text-accent-500/70">· ${this.escapeHtml(this.inferredLabel())}</span>`
      : ""
    pill.innerHTML = `
      <span class="max-w-[160px] truncate">${this.escapeHtml(display)}</span>${suffix}
      <button type="button" class="flex-shrink-0 text-accent-500 hover:text-accent-700" data-action="click->contact-pill-input#removePill" tabindex="-1">
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
      </button>
    `
    this.pillsTarget.insertBefore(pill, this.searchTarget)
    this.updateHidden()
  }

  // The suffix label — read from a data attribute so it stays localized; falls
  // back to English for the rare case the attribute is absent.
  inferredLabel() {
    return this.element.dataset.contactPillInputInferredLabel || "inferred"
  }

  removePill(event) {
    this._clearInferred()
    const pill = event.currentTarget.closest("[data-email]")
    if (pill) {
      pill.remove()
      this.updateHidden()
    }
  }

  updateHidden() {
    const emails = []
    this.pillsTarget.querySelectorAll("[data-email]").forEach(pill => {
      emails.push(pill.dataset.email)
    })
    this.hiddenTarget.value = emails.join(", ")
    // Pills mutate the hidden input programmatically, which fires no native
    // event — announce it so listeners (draft autosave) see recipient edits.
    if (!this._initializing) {
      this.hiddenTarget.dispatchEvent(new Event("input", { bubbles: true }))
    }
  }

  // --- Keyboard ---

  handleKeydown(event) {
    if (event.key === "Backspace" && this.searchTarget.value === "") {
      this.removeLastPill()
      return
    }

    const dropdownOpen = !this.dropdownTarget.classList.contains("hidden")
    const items = this.dropdownTarget.querySelectorAll("button")

    // Enter must never bubble to the compose <form> — that would submit and SEND
    // the email. Instead it picks the highlighted suggestion, or commits whatever
    // address the user has typed as a pill.
    if (event.key === "Enter") {
      event.preventDefault()
      if (dropdownOpen && this.selectedIndex >= 0 && items[this.selectedIndex]) {
        this.selectResult(items[this.selectedIndex])
      } else {
        this.commitTypedValue()
      }
      return
    }

    if (dropdownOpen) {
      if (event.key === "ArrowDown") {
        event.preventDefault()
        this.selectedIndex = Math.min(this.selectedIndex + 1, items.length - 1)
        this.highlightItem(items)
      } else if (event.key === "ArrowUp") {
        event.preventDefault()
        this.selectedIndex = Math.max(this.selectedIndex - 1, 0)
        this.highlightItem(items)
      } else if (event.key === "Escape") {
        this.hideDropdown()
      }
    }
  }

  // Turn the free text in the search box into a pill (used on Enter). Trailing
  // separators are stripped so "a@b.com," commits cleanly.
  commitTypedValue() {
    const value = this.searchTarget.value.trim().replace(/[,;]+$/, "").trim()
    if (!value) return
    this.addPill(value, value)
    this.searchTarget.value = ""
    this.hideDropdown()
  }

  highlightItem(items) {
    items.forEach((item, i) => {
      if (i === this.selectedIndex) {
        item.classList.add("bg-accent-50")
      } else {
        item.classList.remove("bg-accent-50")
      }
    })
  }

  removeLastPill() {
    const pills = this.pillsTarget.querySelectorAll("[data-email]")
    if (pills.length > 0) {
      pills[pills.length - 1].remove()
      this.updateHidden()
    }
  }

  // --- Paste ---

  handlePaste(event) {
    const text = event.clipboardData?.getData("text")
    if (!text) return

    const emailRegex = /[\w.+-]+@[\w-]+\.[\w.-]+/g
    const matches = text.match(emailRegex)
    if (matches) {
      event.preventDefault()
      matches.forEach(email => this.addPill(email, email))
      this.searchTarget.value = ""
    }
  }

  // --- Click outside ---

  setupClickOutside() {
    this.clickOutsideHandler = (event) => {
      if (!this.element.contains(event.target)) {
        this.hideDropdown()
      }
    }
    document.addEventListener("click", this.clickOutsideHandler)
  }

  // --- Helpers ---

  escapeHtml(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }
}

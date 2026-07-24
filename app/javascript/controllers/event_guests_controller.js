import { Controller } from "@hotwired/stimulus"

// Manages the guest invite pill-input on the calendar event form.
//
// Architecture:
//   - Server-rendered rows (data-event-guests-target="row") represent guests
//     already on the event. Their emails seed the hidden field on connect.
//   - Newly typed/selected guests become pill spans inside the pills container.
//   - The hidden input always holds the full comma-separated list (rows + pills)
//     so the form submission is correct even without JS interaction.
//
// Unlike ContactPillInput, pills are NOT created from the hidden seed on connect —
// the server-rendered rows already display the saved guests visually.

export default class extends Controller {
  static targets = ["search", "hidden", "dropdown", "pills", "row", "removeBtn"]
  static values  = { url: String, inviteLabel: String }

  connect() {
    this.debounceTimer  = null
    this.selectedIndex  = -1
    this.blurTimer      = null
    this.setupClickOutside()
    // Sync hidden from initial rows so any pills added later stack correctly.
    this.sync()
  }

  disconnect() {
    document.removeEventListener("click", this.clickOutsideHandler)
    clearTimeout(this.debounceTimer)
    clearTimeout(this.blurTimer)
  }

  // ── Search ───────────────────────────────────────────────────────────────

  search() {
    clearTimeout(this.debounceTimer)
    const query = this.searchTarget.value.trim()

    if (query.length < 2) {
      this.hideDropdown()
      return
    }

    this.debounceTimer = setTimeout(() => this.fetchResults(query), 200)
  }

  fetchResults(query) {
    const url = this.urlValue || "/contacts/search"
    fetch(`${url}?q=${encodeURIComponent(query)}`, {
      headers: { Accept: "application/json" }
    })
      .then(r => r.json())
      .then(data => this.renderDropdown(data, query))
      .catch(() => this.hideDropdown())
  }

  renderDropdown(results, query) {
    query = query ?? this.searchTarget.value.trim()
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    const showInvite = emailRegex.test(query) && !this.isDupe(query)

    if ((!results || results.length === 0) && !showInvite) {
      this.hideDropdown()
      return
    }

    this.selectedIndex = -1
    const items = (results || []).map((c, i) => {
      const name     = c.display_name || c.name || c.email
      const initials = this.getInitials(name)
      const emailHtml = c.email !== name
        ? `<span class="text-xs text-muted-foreground ml-1">${this.escapeHtml(c.email)}</span>`
        : ""
      return `<button type="button" role="option"
        data-index="${i}"
        data-email="${this.escapeHtml(c.email)}"
        data-display="${this.escapeHtml(name)}"
        class="w-full text-left flex items-center gap-2.5 px-2.5 py-1.5 text-sm hover:bg-muted transition-colors">
        <span class="w-6 h-6 rounded-full bg-subtle flex items-center justify-center text-[10.5px] font-semibold text-muted-foreground flex-none">${this.escapeHtml(initials)}</span>
        <span class="flex-1 min-w-0 text-foreground font-medium">${this.escapeHtml(name)}${emailHtml}</span>
      </button>`
    })

    if (showInvite) {
      const idx = (results || []).length
      items.push(`<button type="button" role="option"
        data-index="${idx}"
        data-email="${this.escapeHtml(query)}"
        data-display="${this.escapeHtml(query)}"
        data-invite="true"
        class="w-full text-left flex items-center gap-2.5 px-2.5 py-1.5 text-sm hover:bg-muted transition-colors">
        <span class="w-6 h-6 rounded-full bg-subtle flex items-center justify-center text-[10.5px] font-semibold text-muted-foreground flex-none">@</span>
        <span class="text-xs text-muted-foreground">${this.escapeHtml(this.inviteLabelValue)} ${this.escapeHtml(query)}</span>
      </button>`)
    }

    this.dropdownTarget.innerHTML = items.join("")
    this.dropdownTarget.querySelectorAll("button").forEach(btn => {
      btn.addEventListener("mousedown", e => {
        e.preventDefault()
        this.selectResult(btn)
      })
    })
    this.dropdownTarget.classList.remove("hidden")
  }

  selectResult(btn) {
    const email   = btn.dataset.email
    const display = btn.dataset.display || email
    this.addGuest(email, display)
    this.searchTarget.value = ""
    this.hideDropdown()
  }

  hideDropdown() {
    this.dropdownTarget.classList.add("hidden")
    this.dropdownTarget.innerHTML = ""
    this.selectedIndex = -1
  }

  // ── Guest management ─────────────────────────────────────────────────────

  addGuest(email, display) {
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    if (!emailRegex.test(email)) return
    if (this.isDupe(email)) return

    const pill = document.createElement("span")
    pill.className =
      "bg-subtle rounded-[7px] px-2 py-0.5 text-xs font-medium inline-flex items-center gap-1 whitespace-nowrap"
    pill.dataset.pillEmail = email
    pill.innerHTML = `
      <span class="max-w-[160px] truncate">${this.escapeHtml(display || email)}</span>
      <button type="button"
              class="inline-flex p-0.5 text-muted-foreground hover:text-foreground rounded"
              data-action="click->event-guests#removePill"
              tabindex="-1">
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24" stroke-width="2.2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
        </svg>
      </button>
    `
    this.pillsTarget.insertBefore(pill, this.searchTarget)
    this.sync()
  }

  // Removes a server-rendered guest row (the "x" button inside GuestList).
  removeGuest(event) {
    const row = event.currentTarget.closest("[data-event-guests-email]")
    if (row) {
      row.remove()
      this.sync()
    }
  }

  // Removes a client-side pill (newly added, not yet saved).
  removePill(event) {
    const pill = event.currentTarget.closest("[data-pill-email]")
    if (pill) {
      pill.remove()
      this.sync()
    }
  }

  // Writes all current guest emails (rows + pills) into the hidden field.
  sync() {
    const rowEmails = this.hasRowTarget
      ? this.rowTargets.map(r => r.dataset.eventGuestsEmail).filter(Boolean)
      : []
    const pillEmails = Array.from(
      this.pillsTarget.querySelectorAll("[data-pill-email]")
    ).map(p => p.dataset.pillEmail)

    this.hiddenTarget.value = [...rowEmails, ...pillEmails].join(",")
  }

  // ── Keyboard navigation ───────────────────────────────────────────────────

  handleKeydown(event) {
    const dropdownOpen = !this.dropdownTarget.classList.contains("hidden")
    const items        = this.dropdownTarget.querySelectorAll("button")

    switch (event.key) {
      case "ArrowDown":
        if (dropdownOpen) {
          event.preventDefault()
          this.selectedIndex = Math.min(this.selectedIndex + 1, items.length - 1)
          this.highlightItem(items)
        }
        break

      case "ArrowUp":
        if (dropdownOpen) {
          event.preventDefault()
          this.selectedIndex = Math.max(this.selectedIndex - 1, 0)
          this.highlightItem(items)
        }
        break

      case "Enter":
        event.preventDefault()
        if (dropdownOpen && this.selectedIndex >= 0 && items[this.selectedIndex]) {
          this.selectResult(items[this.selectedIndex])
        } else {
          this.commitTyped()
        }
        break

      case ",":
        event.preventDefault()
        this.commitTyped()
        break

      case "Backspace":
        if (this.searchTarget.value === "") {
          this.removeLastPill()
        }
        break

      case "Escape":
        this.hideDropdown()
        break
    }
  }

  handleBlur() {
    // After a short delay (allows mousedown on a dropdown item to fire first),
    // commit any valid email address left in the input so typed addresses are
    // not silently discarded when the user clicks the save button.
    clearTimeout(this.blurTimer)
    this.blurTimer = setTimeout(() => this.commitTyped(), 150)
  }

  commitTyped() {
    const value = this.searchTarget.value.trim().replace(/[,;]+$/, "").trim()
    if (!value) return
    const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
    if (emailRegex.test(value)) {
      this.addGuest(value, value)
      this.searchTarget.value = ""
      this.hideDropdown()
    }
  }

  highlightItem(items) {
    items.forEach((item, i) => {
      item.classList.toggle("bg-muted", i === this.selectedIndex)
    })
  }

  removeLastPill() {
    const pills = this.pillsTarget.querySelectorAll("[data-pill-email]")
    if (pills.length > 0) {
      pills[pills.length - 1].remove()
      this.sync()
    }
  }

  // ── Click-outside ─────────────────────────────────────────────────────────

  setupClickOutside() {
    this.clickOutsideHandler = event => {
      if (!this.element.contains(event.target)) {
        this.hideDropdown()
      }
    }
    document.addEventListener("click", this.clickOutsideHandler)
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  isDupe(email) {
    const lower    = email.toLowerCase()
    const rowLower = this.hasRowTarget
      ? this.rowTargets.map(r => (r.dataset.eventGuestsEmail || "").toLowerCase())
      : []
    const pillLower = Array.from(
      this.pillsTarget.querySelectorAll("[data-pill-email]")
    ).map(p => p.dataset.pillEmail.toLowerCase())
    return [...rowLower, ...pillLower].includes(lower)
  }

  getInitials(name) {
    return name
      .split(/\s+/)
      .slice(0, 2)
      .map(w => (w[0] || "").toUpperCase())
      .join("")
  }

  // Escapes for BOTH text and double-quoted attribute contexts (the results
  // are interpolated into data-* attributes, so quotes must not break out).
  escapeHtml(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML.replace(/"/g, "&quot;")
  }
}

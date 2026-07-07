import { Controller } from "@hotwired/stimulus"

// Drives the inbox search bar. The <form> itself navigates the
// `email_search_results` Turbo Frame (data-turbo-frame), so there is no manual
// fetch here — this controller only decides *when* to submit and toggles the
// filter panel.
//
// - text input: debounced submit on `input`, immediate on Enter
// - any filter change (select / checkbox / toggle / date): immediate submit,
//   wired at the form level via `change->email-search#submitNow`
// - Filters button: toggles the panel
// - tag filter box: client-side show/hide of the tag checkboxes
// - modifier typeahead: Gmail-style token detection, shows a suggestions panel
// - busy state: while a fetch is in flight, shows the search-bar progress bar +
//   spinner and flags the results pane (data-searching) so the skeleton
//   placeholder replaces the stale list (see _showBusy/_hideBusy)
// - empty box + no filters: leaves search and visits the real inbox instead of
//   submitting a blank in-frame search (see _performSubmit)
export default class extends Controller {
  static targets = ["query", "filterPanel", "tagOption", "suggestions", "suggestionsList", "searchIcon", "spinner", "progress"]
  static values = {
    debounce: { type: Number, default: 300 },
    suggestions: { type: Array, default: [] },
    inboxUrl: String
  }

  connect() {
    this.frame = document.getElementById("email_search_results")
    // Wrapper around the results frame — carries data-searching to swap the stale
    // list for the skeleton placeholder while a fetch is in flight.
    this.pane = document.getElementById("email_search_pane")
    this._activeIndex = -1
    this._blurTimer = null
    this._remoteTimer = null
    this._openPanel = false

    if (this.frame) {
      this._onFetchStart = (e) => { if (e.target === this.frame) this._showBusy() }
      this._onFetchDone  = (e) => { if (e.target === this.frame) this._hideBusy() }
      this._onFetchError = (e) => { if (e.target === this.frame) this._hideBusy() }
      this.frame.addEventListener("turbo:before-fetch-request", this._onFetchStart)
      this.frame.addEventListener("turbo:frame-render",          this._onFetchDone)
      this.frame.addEventListener("turbo:fetch-request-error",   this._onFetchError)
    }
  }

  disconnect() {
    clearTimeout(this.timer)
    clearTimeout(this._blurTimer)
    clearTimeout(this._remoteTimer)
    if (this.frame) {
      this.frame.removeEventListener("turbo:before-fetch-request", this._onFetchStart)
      this.frame.removeEventListener("turbo:frame-render",          this._onFetchDone)
      this.frame.removeEventListener("turbo:fetch-request-error",   this._onFetchError)
    }
    this._hideBusy()
  }

  // --- Submit scheduling ---

  scheduleSubmit() {
    this._scheduleSubmitOnly()
    this._onInput()
  }

  // Debounced submit without re-running suggestion detection — used after a
  // suggestion is applied so the panel doesn't immediately reopen.
  _scheduleSubmitOnly() {
    clearTimeout(this.timer)
    this.timer = setTimeout(() => this._performSubmit(), this.debounceValue)
  }

  submitNow() {
    clearTimeout(this.timer)
    this._performSubmit()
  }

  // Empty box + no active filters → this isn't a search any more. Leave the
  // search frame entirely and restore the *real* inbox (grouped list, view
  // switcher, feed) with a top-level visit — instead of submitting a blank
  // search that renders a bare thread list inside the frame and strands you on
  // /search. Any active filter keeps us in filtered search.
  _performSubmit() {
    if (this._isBlankSearch() && this.hasInboxUrlValue && window.Turbo) {
      window.Turbo.visit(this.inboxUrlValue, { action: "replace" })
      return
    }
    this.element.requestSubmit()
  }

  _isBlankSearch() {
    const q = this.hasQueryTarget ? this.queryTarget.value.trim() : ""
    if (q !== "") return false
    return !this._hasActiveFilters()
  }

  // Any filter field carrying a real value — folder/category/priority selects,
  // sender/domain/date inputs, account/tag checkboxes, the unread & attachment
  // toggles. Skips the query itself, the tag AND/OR mode, pagination, and the
  // "empty" sentinels a control submits when it isn't really filtering.
  _hasActiveFilters() {
    const skipKeys = new Set(["q", "tag_match", "page"])
    const emptyVals = new Set(["", "0", "all", "false"])
    for (const [key, value] of new FormData(this.element).entries()) {
      if (skipKeys.has(key)) continue
      if (emptyVals.has(String(value).trim().toLowerCase())) continue
      return true
    }
    return false
  }

  // --- Keydown handling ---

  handleKeydown(event) {
    const panelOpen = this._panelOpen()

    if (event.key === "Escape") {
      if (panelOpen) {
        this._closePanel()
        event.preventDefault()
      }
      return
    }

    if (event.key === "ArrowDown") {
      if (panelOpen) {
        event.preventDefault()
        this._moveHighlight(1)
      }
      return
    }

    if (event.key === "ArrowUp") {
      if (panelOpen) {
        event.preventDefault()
        this._moveHighlight(-1)
      }
      return
    }

    if (event.key === "Tab") {
      if (panelOpen && this._activeIndex >= 0) {
        event.preventDefault()
        this._applyHighlighted()
      }
      return
    }

    if (event.key === "Enter") {
      if (panelOpen && this._activeIndex >= 0) {
        event.preventDefault()
        this._applyHighlighted()
      } else {
        event.preventDefault()
        this.submitNow()
      }
    }
  }

  // --- Filter panel ---

  toggleFilters() {
    this.filterPanelTarget.classList.toggle("hidden")
  }

  // Client-side filter of the tag checkbox list — no request.
  filterTags(event) {
    const term = event.target.value.trim().toLowerCase()
    this.tagOptionTargets.forEach((el) => {
      const name = el.dataset.tagName || ""
      el.classList.toggle("hidden", term !== "" && !name.includes(term))
    })
  }

  // --- Suggestions: open / close ---

  openSuggestions() {
    clearTimeout(this._blurTimer)
    this._onInput()
  }

  closeSuggestionsSoon() {
    // Give mousedown on a row time to fire before blur hides the panel.
    this._blurTimer = setTimeout(() => this._closePanel(), 120)
  }

  // --- Private ---

  _onInput() {
    const { token, prefix, partial } = this._tokenAtCaret()
    if (!token) {
      // No modifier token — show modifier catalog filtered by what's typed.
      this._renderModifierMode(partial)
    } else {
      // Known modifier prefix — show value options.
      this._renderValueMode(prefix, partial, token)
    }
  }

  // Detect the modifier token currently under/before the caret.
  // Returns { token: CatalogEntry|null, prefix: "from:", partial: "ac" }.
  _tokenAtCaret() {
    const input = this.queryTarget
    const textBefore = input.value.slice(0, input.selectionStart)
    // Find the start of the current whitespace-delimited token.
    const lastSpace = textBefore.lastIndexOf(" ")
    const current = textBefore.slice(lastSpace + 1)

    const colon = current.indexOf(":")
    if (colon <= 0) {
      // Plain text — modifier mode filtering by current token.
      return { token: null, prefix: null, partial: current.toLowerCase() }
    }

    const modifierKey = current.slice(0, colon + 1) // e.g. "from:"
    const partial     = current.slice(colon + 1)    // e.g. "ac"
    const token       = this._catalogEntry(modifierKey)
    return { token, prefix: modifierKey, partial }
  }

  _catalogEntry(prefix) {
    const lp = prefix.toLowerCase()
    return this.suggestionsValue.find((e) => e.token.toLowerCase() === lp) || null
  }

  // MODE 1: Show catalog entries (or a filtered subset) as modifier suggestions.
  _renderModifierMode(partial) {
    const entries = this.suggestionsValue.filter((e) =>
      partial === "" || e.token.toLowerCase().startsWith(partial)
    )
    if (!entries.length) { this._closePanel(); return }

    this._clearRows()
    this._appendHeading()

    entries.forEach((entry, i) => {
      const btn = this._makeElement("button", {
        type: "button",
        role: "option",
        id: `email-search-suggestion-${i}`,
        "aria-selected": "false",
        class: "w-full text-left px-2.5 py-1.5 flex items-center gap-2 hover:bg-muted transition-colors"
      })

      const tokenSpan = this._makeElement("span", {
        class: "font-mono text-[11px] font-medium text-gray-700 dark:text-gray-200 flex-shrink-0 w-24"
      })
      tokenSpan.textContent = entry.token

      const descSpan = this._makeElement("span", {
        class: "text-[10px] text-gray-500 dark:text-gray-400 truncate"
      })
      descSpan.textContent = entry.description

      btn.appendChild(tokenSpan)
      btn.appendChild(descSpan)

      btn.addEventListener("mousedown", (e) => {
        e.preventDefault()
        this._applyModifier(entry.token)
      })

      this.suggestionsListTarget.appendChild(btn)
    })

    this._setActiveIndex(-1)
    this._openPanel = true
    this.suggestionsTarget.classList.remove("hidden")
    this.queryTarget.setAttribute("aria-expanded", "true")
  }

  // MODE 2: Show value options for a known modifier.
  _renderValueMode(prefix, partial, token) {
    const type = token.type

    if (type === "text" || type === "date") {
      // Show a non-interactive hint row.
      this._clearRows()
      const hint = this._makeElement("div", {
        class: "px-2.5 py-2 text-[10px] text-gray-400 dark:text-gray-500"
      })
      const descSpan = this._makeElement("span")
      descSpan.textContent = token.description
      hint.appendChild(descSpan)
      if (token.hint) {
        const hintSpan = this._makeElement("span", { class: "ml-1 font-mono" })
        hintSpan.textContent = "(" + token.hint + ")"
        hint.appendChild(hintSpan)
      }
      this.suggestionsListTarget.appendChild(hint)
      this._setActiveIndex(-1)
      this._openPanel = true
      this.suggestionsTarget.classList.remove("hidden")
      this.queryTarget.setAttribute("aria-expanded", "true")
      return
    }

    if (type === "enum") {
      const values = (token.values || []).filter((v) =>
        partial === "" || v.label.toLowerCase().includes(partial.toLowerCase()) || v.value.toLowerCase().includes(partial.toLowerCase())
      )
      if (!values.length) { this._closePanel(); return }

      this._clearRows()
      values.forEach((v, i) => {
        const btn = this._makeElement("button", {
          type: "button",
          role: "option",
          id: `email-search-suggestion-${i}`,
          "aria-selected": "false",
          class: "w-full text-left px-2.5 py-1.5 flex items-center gap-2 hover:bg-muted transition-colors"
        })

        const labelSpan = this._makeElement("span", { class: "text-xs text-gray-700 dark:text-gray-200 flex-shrink-0" })
        labelSpan.textContent = v.label

        btn.appendChild(labelSpan)

        // Show raw value muted when it differs from the label.
        if (v.label !== v.value) {
          const valSpan = this._makeElement("span", { class: "text-[10px] text-gray-400 dark:text-gray-500 font-mono" })
          valSpan.textContent = v.value
          btn.appendChild(valSpan)
        }

        btn.addEventListener("mousedown", (e) => {
          e.preventDefault()
          this._applyValue(prefix, v.value)
        })

        this.suggestionsListTarget.appendChild(btn)
      })

      this._setActiveIndex(-1)
      this._openPanel = true
      this.suggestionsTarget.classList.remove("hidden")
      this.queryTarget.setAttribute("aria-expanded", "true")
      return
    }

    if (type === "remote") {
      if (partial.length < 2) {
        // Show a hint row while waiting for 2+ chars.
        this._clearRows()
        const hint = this._makeElement("div", { class: "px-2.5 py-2 text-[10px] text-gray-400 dark:text-gray-500" })
        const sp = this._makeElement("span")
        sp.textContent = token.description
        hint.appendChild(sp)
        this.suggestionsListTarget.appendChild(hint)
        this._setActiveIndex(-1)
        this._openPanel = true
        this.suggestionsTarget.classList.remove("hidden")
        this.queryTarget.setAttribute("aria-expanded", "true")
        return
      }

      clearTimeout(this._remoteTimer)
      this._remoteTimer = setTimeout(() => {
        this._fetchRemote(token.url, partial, prefix)
      }, 200)
    }
  }

  _fetchRemote(url, q, prefix) {
    fetch(`${url}?q=${encodeURIComponent(q)}`, { headers: { Accept: "application/json" } })
      .then((r) => r.json())
      .then((data) => {
        // The user closed the panel (Escape/blur) while the fetch was in
        // flight — a stale response must not reopen it.
        if (!this._openPanel) return
        this._clearRows()
        if (!data.length) { this._closePanel(); return }

        data.forEach((c, i) => {
          const name  = c.display_name || c.name || c.email || ""
          const email = c.email || ""

          const btn = this._makeElement("button", {
            type: "button",
            role: "option",
            id: `email-search-suggestion-${i}`,
            "aria-selected": "false",
            class: "w-full text-left px-2.5 py-1.5 flex items-center gap-2 hover:bg-muted transition-colors"
          })

          const nameSpan = this._makeElement("span", { class: "text-xs text-gray-700 dark:text-gray-200 truncate" })
          nameSpan.textContent = name

          btn.appendChild(nameSpan)

          if (email) {
            const emailSpan = this._makeElement("span", { class: "text-[10px] text-gray-400 dark:text-gray-500 truncate" })
            emailSpan.textContent = email
            btn.appendChild(emailSpan)
          }

          const value = email || name
          btn.addEventListener("mousedown", (ev) => {
            ev.preventDefault()
            this._applyValue(prefix, value)
          })

          this.suggestionsListTarget.appendChild(btn)
        })

        this._setActiveIndex(-1)
        this._openPanel = true
        this.suggestionsTarget.classList.remove("hidden")
        this.queryTarget.setAttribute("aria-expanded", "true")
      })
      .catch(() => this._closePanel())
  }

  // Insert/replace the current token with the chosen modifier (e.g. "from:").
  _applyModifier(modifierToken) {
    const input      = this.queryTarget
    const before     = input.value.slice(0, input.selectionStart)
    const after      = input.value.slice(input.selectionStart)
    const lastSpace  = before.lastIndexOf(" ")
    const prefix     = before.slice(0, lastSpace + 1) // text before current token
    const newValue   = prefix + modifierToken
    input.value = newValue + after.replace(/^\S*/, "")
    const pos = newValue.length
    input.setSelectionRange(pos, pos)
    input.focus()
    this._closePanel()
    // Re-run detection — now in value mode.
    this._onInput()
  }

  // Complete the current modifier token with the selected value + trailing space.
  _applyValue(prefix, value) {
    const input     = this.queryTarget
    const before    = input.value.slice(0, input.selectionStart)
    const after     = input.value.slice(input.selectionStart)
    const lastSpace = before.lastIndexOf(" ")
    const textBefore = before.slice(0, lastSpace + 1)

    // Quote the value if it contains spaces.
    const quoted   = value.includes(" ") ? `"${value}"` : value
    const newToken = prefix + quoted
    // Trim the current token from `after` then append the rest.
    const newValue = textBefore + newToken + " " + after.replace(/^\S*/, "").trimStart()
    input.value = newValue
    const pos = (textBefore + newToken + " ").length
    input.setSelectionRange(pos, pos)
    input.focus()
    this._closePanel()
    this._scheduleSubmitOnly()
  }

  _applyHighlighted() {
    const rows = this._optionRows()
    if (this._activeIndex < 0 || this._activeIndex >= rows.length) return
    rows[this._activeIndex].dispatchEvent(new MouseEvent("mousedown", { bubbles: true }))
  }

  _moveHighlight(delta) {
    const rows = this._optionRows()
    if (!rows.length) return
    const next = (this._activeIndex + delta + rows.length) % rows.length
    this._setActiveIndex(next)
    rows[next].scrollIntoView({ block: "nearest" })
  }

  _setActiveIndex(i) {
    const rows = this._optionRows()
    rows.forEach((r, idx) => {
      const active = idx === i
      r.setAttribute("aria-selected", String(active))
      r.classList.toggle("bg-muted", active)
    })
    this._activeIndex = i
    const activeId = i >= 0 && rows[i] ? rows[i].id : ""
    this.queryTarget.setAttribute("aria-activedescendant", activeId)
  }

  _optionRows() {
    return Array.from(this.suggestionsListTarget.querySelectorAll("[role=option]"))
  }

  _closePanel() {
    this._openPanel = false
    this._activeIndex = -1
    if (this.hasSuggestionsTarget) {
      this.suggestionsTarget.classList.add("hidden")
    }
    if (this.hasQueryTarget) {
      this.queryTarget.setAttribute("aria-expanded", "false")
      this.queryTarget.removeAttribute("aria-activedescendant")
    }
  }

  _panelOpen() {
    return this._openPanel && this.hasSuggestionsTarget && !this.suggestionsTarget.classList.contains("hidden")
  }

  _clearRows() {
    if (this.hasSuggestionsListTarget) {
      while (this.suggestionsListTarget.firstChild) {
        this.suggestionsListTarget.removeChild(this.suggestionsListTarget.firstChild)
      }
    }
  }

  _appendHeading() {
    // Small uppercase label at the top of the modifier list.
    const h = this._makeElement("div", {
      class: "text-[9px] uppercase tracking-wide text-gray-400 px-2.5 pt-1 pb-0.5 select-none",
      "aria-hidden": "true"
    })
    // Static heading text — set as attribute to avoid innerHTML with user data.
    h.textContent = this.element.dataset.emailSearchHeadingText || "Refine your search"
    this.suggestionsListTarget.appendChild(h)
  }

  // --- Busy state ---

  _showBusy() {
    if (this.hasSearchIconTarget) this.searchIconTarget.classList.add("hidden")
    if (this.hasSpinnerTarget)    this.spinnerTarget.classList.remove("hidden")
    if (this.hasProgressTarget)   this.progressTarget.classList.remove("hidden")
    if (this.pane)                this.pane.setAttribute("data-searching", "on")
  }

  _hideBusy() {
    if (this.hasSearchIconTarget) this.searchIconTarget.classList.remove("hidden")
    if (this.hasSpinnerTarget)    this.spinnerTarget.classList.add("hidden")
    if (this.hasProgressTarget)   this.progressTarget.classList.add("hidden")
    if (this.pane)                this.pane.removeAttribute("data-searching")
  }

  // --- DOM helpers ---

  // Build an element with attributes safely (no innerHTML with user data).
  _makeElement(tag, attrs = {}) {
    const el = document.createElement(tag)
    Object.entries(attrs).forEach(([k, v]) => el.setAttribute(k, v))
    return el
  }
}

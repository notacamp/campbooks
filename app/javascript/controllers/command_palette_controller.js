import { Controller } from "@hotwired/stimulus"
import { CommandEngine, ICONS, SPINNER } from "lib/command_items"

// The classic Cmd+K command palette. All of the item model, filtering, search
// and composite-capture logic lives in the shared CommandEngine (lib/command_items.js),
// which the Scout overlay reuses; this controller is the host — it owns only the
// dialog, the DOM rendering of the list/breadcrumb, and the input element.
export default class extends Controller {
  static targets = ["dialog", "input", "list", "breadcrumb"]
  static values = {
    context: { type: String, default: "" },
    // UUID string (Number would truncate "95be…" to 95 and break every action)
    messageId: { type: String, default: "" },
    subject: { type: String, default: "" },
    folders: { type: Array, default: [] },
    commands: { type: Array, default: [] },
    open: { type: Boolean, default: false }
  }

  connect() {
    this.engine = new CommandEngine(this._host())
    this.boundKeydown = this._handleKeydown.bind(this)
    this.boundDialogClick = this._handleDialogClick.bind(this)
    this.boundMouseMove = this._handleMouseMove.bind(this)
    document.addEventListener("keydown", this.boundKeydown)
    this._searchPlaceholder = this.hasInputTarget ? this.inputTarget.getAttribute("placeholder") : ""
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKeydown)
    this.engine.reset()
  }

  // The host interface the CommandEngine calls back into for DOM touchpoints.
  _host() {
    return {
      rawQuery: () => this.inputTarget.value,
      config: () => ({
        context: this.contextValue,
        messageId: this.messageIdValue,
        subject: this.subjectValue,
        folders: this.foldersValue,
        commands: this.hasCommandsValue ? this.commandsValue : []
      }),
      leadItems: () => [],
      post: (url, params) => this._post(url, params),
      visit: (url) => Turbo.visit(url),
      close: () => this.close(),
      render: () => this._render(),
      updateSelection: () => this._updateSelection(),
      scrollToSelected: () => this._scrollToSelected(),
      renderBreadcrumb: () => this._renderBreadcrumb(),
      setInputValue: (v) => { this.inputTarget.value = v },
      setPlaceholder: (p) => { this.inputTarget.placeholder = p },
      focusInput: () => this.inputTarget.focus(),
      searchPlaceholder: () => this._searchPlaceholder
    }
  }

  // --- Open / Close ---

  _handleKeydown(event) {
    if (this.openValue) {
      this.engine.onKeydown(event)
      return
    }
    if ((event.metaKey || event.ctrlKey) && event.key === "k") {
      event.preventDefault()
      this.open()
    }
  }

  open() {
    this.openValue = true
    this.engine.reset()
    this.dialogTarget.showModal()
    this.dialogTarget.addEventListener("click", this.boundDialogClick)
    this.listTarget.addEventListener("mousemove", this.boundMouseMove)
    this.inputTarget.value = ""
    this.inputTarget.placeholder = this._searchPlaceholder
    this._renderBreadcrumb()
    this.inputTarget.focus()
    this._render()
  }

  close() {
    this.openValue = false
    this.engine.reset()
    this._renderBreadcrumb()
    this.inputTarget.placeholder = this._searchPlaceholder
    this.dialogTarget.removeEventListener("click", this.boundDialogClick)
    this.listTarget.removeEventListener("mousemove", this.boundMouseMove)
    this.dialogTarget.close()
  }

  _handleDialogClick(event) {
    if (event.target === this.dialogTarget) {
      this.close()
    }
  }

  // --- Input ---

  filter() {
    this.engine.onInput()
  }

  // Delegated from the list container: resolve the clicked row from the event path.
  selectItem(event) {
    const row = event.target.closest("[data-index]")
    if (!row) return
    this.engine.selectByIndex(parseInt(row.dataset.index))
  }

  // Delegated hover (mouseover bubbles; mouseenter does not).
  hoverItem(event) {
    const row = event.target.closest("[data-index]")
    if (!row) return
    this.engine.hoverByIndex(parseInt(row.dataset.index))
  }

  _handleMouseMove() {
    this.engine.keyboardNav = false
  }

  // --- Render ---

  _render() {
    const items = this.engine.currentItems()
    this.engine.clampSelection(items.length)

    if (items.length === 0) {
      this.inputTarget.removeAttribute("aria-activedescendant")
      this.listTarget.innerHTML = this.engine.isLoading() ? this._loadingRow() : this._emptyState()
      return
    }

    const grouped = new Map()
    items.forEach((item, i) => {
      if (!grouped.has(item.group)) grouped.set(item.group, [])
      grouped.get(item.group).push({ item, index: i })
    })

    let html = ""
    for (const [group, rows] of grouped) {
      html += `<div class="px-2 pt-2 pb-1 text-[10px] font-semibold text-gray-400 uppercase tracking-wider">${this._esc(group)}</div>`
      for (const { item, index } of rows) {
        html += this._row(item, index)
      }
    }
    if (this.engine.isLoading()) html += this._loadingRow()

    this.listTarget.innerHTML = html
    this.inputTarget.setAttribute("aria-activedescendant", `cp-item-${this.engine.selectedIndex}`)
    this._scrollToSelected()
  }

  // Rows carry NO per-element listeners: click + hover are delegated to the stable
  // list container (see command_palette.rb). The data-cp-* hooks let
  // _updateSelection recolor a row in place without a rebuild.
  _row(item, index) {
    const selected = index === this.engine.selectedIndex
    const subtitle = item.subtitle
      ? `<span data-cp-sub class="block truncate text-[11px] ${selected ? "text-accent-600/80" : "text-gray-500"}">${this._esc(item.subtitle)}</span>`
      : ""
    return `
      <button type="button"
              id="cp-item-${index}"
              role="option"
              aria-selected="${selected}"
              class="w-full flex items-center gap-3 px-3 py-2 text-left rounded-lg transition-colors ${selected ? "bg-accent-50" : "hover:bg-gray-100"}"
              data-index="${index}">
        <span data-cp-icon class="flex-shrink-0 ${selected ? "text-accent-600" : "text-gray-400"}">${ICONS[item.icon] || ICONS.search}</span>
        <span class="min-w-0 flex-1">
          <span data-cp-title class="block truncate text-xs ${selected ? "text-accent-700 font-medium" : "text-gray-700"}">${this._esc(item.title)}</span>
          ${subtitle}
        </span>
      </button>`
  }

  _loadingRow() {
    return `<div class="flex items-center gap-2 px-3 py-3 text-xs text-gray-400">${SPINNER}<span>Searching…</span></div>`
  }

  _emptyState() {
    const query = this.inputTarget.value.trim()
    if (this.engine.capture) {
      const slot = this.engine._activeSlot()
      const message = query
        ? `No matches for "${this._esc(query)}"`
        : (slot.source === "folders" ? "No folders" : "Type to search…")
      return `<div class="px-4 py-8 text-center text-xs text-gray-400">${message}</div>`
    }
    const message = query ? `No results for "${this._esc(query)}"` : "No matching commands"
    return `<div class="px-4 py-8 text-center text-xs text-gray-400">${message}</div>`
  }

  _renderBreadcrumb() {
    if (!this.hasBreadcrumbTarget) return
    const cap = this.engine.capture
    if (!cap) {
      this.breadcrumbTarget.className = "hidden"
      this.breadcrumbTarget.innerHTML = ""
      return
    }
    const parts = [
      `<span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-md bg-accent-50 text-accent-700 text-xs font-medium whitespace-nowrap">${this._esc(cap.def.label)}</span>`
    ]
    cap.def.slots.forEach((slot, i) => {
      if (i < cap.slotIndex) {
        parts.push(`<span class="inline-flex items-center max-w-[160px] truncate px-2 py-0.5 rounded-md bg-gray-100 text-gray-700 text-xs whitespace-nowrap">${this._esc(cap.values[slot.key].title)}</span>`)
      }
    })
    this.breadcrumbTarget.className = "flex items-center gap-1.5 flex-shrink-0 min-w-0"
    this.breadcrumbTarget.innerHTML = parts.join('<span class="text-gray-300 text-xs">&rsaquo;</span>')
  }

  _scrollToSelected() {
    requestAnimationFrame(() => {
      const btn = this.listTarget.querySelector(`[data-index="${this.engine.selectedIndex}"]`)
      if (btn) btn.scrollIntoView({ block: "nearest" })
    })
  }

  // Move the selection highlight without touching the list's DOM nodes, keeping the
  // delegated handlers and every row's identity stable. Used for hover + arrow keys.
  _updateSelection() {
    const rows = this.listTarget.querySelectorAll("[data-index]")
    if (!rows.length) return
    rows.forEach(row => {
      const selected = parseInt(row.dataset.index) === this.engine.selectedIndex
      row.setAttribute("aria-selected", selected ? "true" : "false")
      row.classList.toggle("bg-accent-50", selected)
      row.classList.toggle("hover:bg-gray-100", !selected)
      this._recolor(row.querySelector("[data-cp-icon]"), selected, ["text-accent-600"], ["text-gray-400"])
      this._recolor(row.querySelector("[data-cp-title]"), selected, ["text-accent-700", "font-medium"], ["text-gray-700"])
      this._recolor(row.querySelector("[data-cp-sub]"), selected, ["text-accent-600/80"], ["text-gray-500"])
    })
    this.inputTarget.setAttribute("aria-activedescendant", `cp-item-${this.engine.selectedIndex}`)
  }

  _recolor(el, selected, onClasses, offClasses) {
    if (!el) return
    onClasses.forEach(c => el.classList.toggle(c, selected))
    offClasses.forEach(c => el.classList.toggle(c, !selected))
  }

  // --- Helpers ---

  _esc(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }

  _post(url, params) {
    const body = new FormData()
    for (const [key, value] of Object.entries(params)) {
      body.append(key, value)
    }
    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    fetch(url, {
      method: "POST",
      headers: {
        "X-CSRF-Token": csrfToken,
        "Accept": "text/vnd.turbo-stream.html, text/html"
      },
      body
    }).then(response => {
      if (response.headers.get("Content-Type")?.includes("text/vnd.turbo-stream.html")) {
        return response.text().then(html => {
          if (html) Turbo.renderStreamMessage(html)
        })
      } else if (response.redirected) {
        Turbo.visit(response.url)
      } else {
        return response.text().then(html => {
          if (html) Turbo.renderStreamMessage(html)
        })
      }
    }).catch(() => {})
  }
}

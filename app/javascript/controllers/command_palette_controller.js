import { Controller } from "@hotwired/stimulus"

const ICONS = {
  grid: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zm10 0a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2V6zM4 16a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2v-2zm10 0a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2z"/></svg>',
  sparkles: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 3v4M3 5h4M6 17v4m-2-2h4m5-16l2.286 6.857L21 12l-5.714 2.143L13 21l-2.286-6.857L5 12l5.714-2.143L13 3z"/></svg>',
  mail: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"/></svg>',
  users: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4.354a4 4 0 110 5.292M15 21H3v-1a6 6 0 0112 0v1zm0 0h6v-1a6 6 0 00-9-5.197M13 7a4 4 0 11-8 0 4 4 0 018 0z"/></svg>',
  "file-text": '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>',
  "chart-bar": '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"/></svg>',
  cog: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/></svg>',
  "at-sign": '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 12a4 4 0 10-8 0 4 4 0 008 0zm0 0v1.5a2.5 2.5 0 005 0V12a9 9 0 10-9 9m4.5-1.206a8.959 8.959 0 01-4.5 1.207"/></svg>',
  pen: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z"/></svg>',
  plus: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/></svg>',
  archive: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/></svg>',
  reply: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6"/></svg>',
  "reply-all": '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 10h5a6 6 0 016 6v2M3 10l6 6m-6-6l6-6m8 14l-6-6 6-6"/></svg>',
  forward: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10h8m0 0l-6 6m6-6l-6-6M3 10h4a6 6 0 016 6v2"/></svg>',
  check: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>',
  folder: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"/></svg>',
  search: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>',
  tag: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 7h.01M7 3h5a1.99 1.99 0 011.414.586l7 7a2 2 0 010 2.828l-5 5a2 2 0 01-2.828 0l-7-7A1.99 1.99 0 013 9V4a1 1 0 011-1z"/></svg>',
  workflow: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2H6a2 2 0 01-2-2V6zm10 10a2 2 0 012-2h2a2 2 0 012 2v2a2 2 0 01-2 2h-2a2 2 0 01-2-2v-2zM10 7h4a2 2 0 012 2v5"/></svg>',
  bell: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9"/></svg>',
  star: '<svg class="w-4 h-4" fill="currentColor" viewBox="0 0 24 24"><path d="M11.48 3.5a.56.56 0 011.04 0l2.12 4.92 5.34.46c.49.04.69.66.31.98l-4.05 3.5 1.21 5.22c.11.48-.41.86-.83.6L12 17.27l-4.63 2.91c-.42.26-.94-.12-.83-.6l1.21-5.22-4.05-3.5c-.38-.32-.18-.94.31-.98l5.34-.46 2.12-4.92z"/></svg>',
  ban: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M18.364 5.636l-12.728 12.728M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/></svg>',
  calendar: '<svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/></svg>'
}

const SPINNER = '<svg class="w-3.5 h-3.5 animate-spin text-gray-400" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path></svg>'

export default class extends Controller {
  static targets = ["dialog", "input", "list", "breadcrumb"]
  static values = {
    context: { type: String, default: "" },
    messageId: { type: Number, default: 0 },
    subject: { type: String, default: "" },
    folders: { type: Array, default: [] },
    commands: { type: Array, default: [] },
    open: { type: Boolean, default: false }
  }

  connect() {
    this.boundKeydown = this._handleKeydown.bind(this)
    this.boundDialogClick = this._handleDialogClick.bind(this)
    this.boundMouseMove = this._handleMouseMove.bind(this)
    document.addEventListener("keydown", this.boundKeydown)
    this.selectedIndex = 0
    this._keyboardNav = false
    this.serverResults = []
    this.searching = false
    this.searchSeq = 0
    this.debounceTimer = null
    this.abortController = null
    this.capture = null
    this._searchPlaceholder = this.hasInputTarget ? this.inputTarget.getAttribute("placeholder") : ""
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKeydown)
    clearTimeout(this.debounceTimer)
    this._abort()
  }

  // --- Commands (instant, client-side) ---

  // Navigation/settings/actions come from the Ruby catalog (real route helpers +
  // permission gating); fall back to a minimal built-in list if absent.
  _catalogCommands() {
    const source = this.hasCommandsValue && this.commandsValue.length ? this.commandsValue : this._fallbackCommands()
    return source.map(c => ({
      name: c.name,
      category: c.category,
      icon: c.icon,
      keywords: c.keywords,
      action: c.method === "post" ? () => this._post(c.url, {}) : () => Turbo.visit(c.url)
    }))
  }

  _fallbackCommands() {
    return [
      { name: "Inbox", category: "Navigate", icon: "mail", url: "/email_messages" },
      { name: "Scout AI Chat", category: "Navigate", icon: "sparkles", url: "/scout" },
      { name: "Documents", category: "Navigate", icon: "file-text", url: "/documents" },
      { name: "Settings", category: "Navigate", icon: "cog", url: "/settings" },
      { name: "Start new email", category: "Actions", icon: "pen", url: "/email_messages/new" }
    ]
  }

  // Composite (parameterized) commands: pick an email/tag/folder inline, then run.
  // Definitions are data; the capture state machine below drives the slots.
  _compositeDefs() {
    return [
      {
        id: "move",
        label: "Move email to folder",
        icon: "folder",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to move…" },
          { key: "folder", label: "Folder", source: "folders", icon: "folder", placeholder: "Move to which folder…" }
        ],
        run: v => this._post("/email_messages/bulk", { tool: "move_to_folder", "email_ids[]": v.email.id, folder_id: v.folder.id })
      },
      {
        id: "tag",
        label: "Tag email",
        icon: "tag",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to tag…" },
          { key: "tag", label: "Tag", source: "search", types: "tags", icon: "tag", placeholder: "Pick a tag…" }
        ],
        run: v => this._post(`/email_messages/${v.email.id}/tool`, { tool: "add_tag", "args[tag_name]": v.tag.title })
      },
      {
        id: "archive",
        label: "Archive email",
        icon: "archive",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to archive…" }
        ],
        run: v => this._post(`/email_messages/${v.email.id}/tool`, { tool: "archive" })
      }
    ]
  }

  _compositeCommands() {
    return this._compositeDefs().map(def => ({
      name: `${def.label}…`,
      category: "Commands",
      icon: def.icon,
      keywords: def.id,
      keepOpen: true, // selecting enters capture mode rather than closing the palette
      action: () => this._enterCapture(def)
    }))
  }

  // Per-message email actions — they operate on the email currently open, so they
  // sit under a "Current email" group with the email's subject as subtitle. That
  // keeps "Move to INVOICES" from reading as a search result when it surfaces.
  _contextCommands() {
    if (this.contextValue === "calendar") return this._calendarContextCommands()

    const commands = []
    if (this.contextValue !== "email-show" || !this.messageIdValue) return commands

    const id = this.messageIdValue
    const group = "Current email"
    const subject = this.subjectValue ? this._truncate(this.subjectValue, 64) : null
    const add = (name, icon, action, subtitle = null) => commands.push({ name, category: group, subtitle, icon, action })

    add("Archive", "archive", () => this._post(`/email_messages/${id}/tool`, { tool: "archive" }))
    add("Reply", "reply", () => this._post(`/email_messages/${id}/compose`, { mode: "reply" }))
    add("Reply all", "reply-all", () => this._post(`/email_messages/${id}/compose`, { mode: "reply_all" }))
    add("Forward", "forward", () => this._post(`/email_messages/${id}/compose`, { mode: "forward" }))
    add("Create calendar event", "calendar", () => this._post(`/email_messages/${id}/tool`, { tool: "create_calendar_event" }), subject)
    add("Dismiss AI todo", "check", () => this._post(`/email_messages/${id}/dismiss_todo`, { _method: "patch" }))
    add("Star sender", "star", () => this._post(`/email_messages/${id}/tool`, { tool: "star_sender" }), subject)
    add("Block sender", "ban", () => this._post(`/email_messages/${id}/tool`, { tool: "block_sender" }), subject)

    for (const f of this.foldersValue || []) {
      if (!f.id) continue
      add(`Move to ${f.name}`, "folder", () => this._post("/email_messages/bulk", { tool: "move_to_folder", "email_ids[]": id, folder_id: f.id }), subject)
    }

    return commands
  }

  // Page-relative calendar nav (previous/next period). Reads the header's prev/next
  // links, whose URLs the server computed for the current view+date.
  _calendarContextCommands() {
    const href = (sel) => document.querySelector(sel)?.getAttribute("href")
    const next = href("[data-calendar-next]")
    const prev = href("[data-calendar-prev]")
    const commands = []
    if (next) commands.push({ name: "Calendar: next period", category: "Calendar", icon: "calendar", action: () => Turbo.visit(next) })
    if (prev) commands.push({ name: "Calendar: previous period", category: "Calendar", icon: "calendar", action: () => Turbo.visit(prev) })
    return commands
  }

  // --- Merged item model (commands + server results share one flat index) ---

  _commandItems() {
    const query = this.inputTarget.value.toLowerCase().trim()
    // Context (current-email) actions lead, then composite commands, then nav/settings.
    const all = [...this._contextCommands(), ...this._compositeCommands(), ...this._catalogCommands()]
    const filtered = !query ? all : all.filter(cmd =>
      cmd.name.toLowerCase().includes(query) ||
      cmd.category.toLowerCase().includes(query) ||
      (cmd.keywords || "").toLowerCase().includes(query)
    )
    return filtered.map(cmd => ({ group: cmd.category, title: cmd.name, subtitle: cmd.subtitle || null, icon: cmd.icon, run: cmd.action, keepOpen: cmd.keepOpen || false }))
  }

  _resultItems() {
    return (this.serverResults || []).map(r => ({
      group: r.type,
      title: r.title,
      subtitle: r.subtitle,
      icon: r.icon,
      run: () => Turbo.visit(r.url)
    }))
  }

  // Candidate items for the active capture slot.
  _captureItems() {
    const slot = this._activeSlot()
    return (this.capture.candidates || []).map(c => ({
      group: slot.label,
      title: c.title,
      subtitle: c.subtitle || null,
      icon: c.icon || slot.icon,
      candidate: c
    }))
  }

  _currentItems() {
    if (this.capture) return this._captureItems()
    return [...this._commandItems(), ...this._resultItems()]
  }

  // --- Open / Close ---

  _handleKeydown(event) {
    if (this.openValue) {
      this._handlePaletteKeydown(event)
      return
    }
    if ((event.metaKey || event.ctrlKey) && event.key === "k") {
      event.preventDefault()
      this.open()
    }
  }

  open() {
    this.openValue = true
    this.selectedIndex = 0
    this._keyboardNav = false
    this.serverResults = []
    this.searching = false
    this.searchSeq = 0
    this.capture = null
    this._renderBreadcrumb()
    this.dialogTarget.showModal()
    this.dialogTarget.addEventListener("click", this.boundDialogClick)
    this.listTarget.addEventListener("mousemove", this.boundMouseMove)
    this.inputTarget.value = ""
    this.inputTarget.placeholder = this._searchPlaceholder
    this.inputTarget.focus()
    this._render()
  }

  close() {
    this.openValue = false
    this.capture = null
    clearTimeout(this.debounceTimer)
    this._abort()
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

  // --- Palette keyboard ---

  _handlePaletteKeydown(event) {
    const items = this._currentItems()
    switch (event.key) {
      case "Escape":
        event.preventDefault()
        if (this.capture) this._exitCapture()
        else this.close()
        break
      case "Backspace":
        // Empty input + capture → step back a slot (chip-style editing).
        if (this.capture && this.inputTarget.value === "") {
          event.preventDefault()
          this._popSlot()
        }
        break
      case "ArrowDown":
        event.preventDefault()
        this._keyboardNav = true
        this.selectedIndex = Math.min(this.selectedIndex + 1, items.length - 1)
        this._updateSelection()
        this._scrollToSelected()
        break
      case "ArrowUp":
        event.preventDefault()
        this._keyboardNav = true
        this.selectedIndex = Math.max(this.selectedIndex - 1, 0)
        this._updateSelection()
        this._scrollToSelected()
        break
      case "Enter": {
        event.preventDefault()
        const item = items[this.selectedIndex]
        if (!item) break
        if (this.capture) {
          if (item.candidate) this._selectCandidate(item.candidate)
        } else {
          item.run()
          if (!item.keepOpen) this.close()
        }
        break
      }
    }
  }

  // --- Search (input handler) ---

  filter() {
    this.selectedIndex = 0
    const query = this.inputTarget.value.trim()
    if (this.capture) {
      this._loadSlotCandidates(query)
    } else {
      this._scheduleSearch(query)
      this._render()
    }
  }

  _scheduleSearch(query) {
    clearTimeout(this.debounceTimer)
    if (query.length < 2) {
      this._abort()
      this.serverResults = []
      this.searching = false
      return
    }
    this.searching = true
    this.debounceTimer = setTimeout(() => this._runSearch(query), 300)
  }

  async _runSearch(query) {
    this._abort()
    const controller = new AbortController()
    this.abortController = controller
    const seq = ++this.searchSeq

    try {
      const response = await fetch(`/search?q=${encodeURIComponent(query)}`, {
        headers: { "Accept": "application/json" },
        signal: controller.signal
      })
      const data = await response.json()
      if (seq !== this.searchSeq || this.capture) return // superseded or mode changed
      this.serverResults = data.results || []
      this.searching = false
      this._render()
    } catch (error) {
      if (error.name === "AbortError" || seq !== this.searchSeq || this.capture) return
      this.serverResults = []
      this.searching = false
      this._render()
    }
  }

  _abort() {
    if (this.abortController) {
      this.abortController.abort()
      this.abortController = null
    }
  }

  // --- Composite capture state machine ---

  _enterCapture(def) {
    this._abort()
    clearTimeout(this.debounceTimer)
    this.capture = { def, slotIndex: 0, values: {}, candidates: [], folderCache: null, loading: false }
    this.serverResults = []
    this.searching = false
    this.selectedIndex = 0
    this.inputTarget.value = ""
    this._syncCaptureSlot()
  }

  _activeSlot() {
    return this.capture.def.slots[this.capture.slotIndex]
  }

  // Re-point the input + breadcrumb at the current slot and load its candidates.
  _syncCaptureSlot() {
    this.inputTarget.placeholder = this._activeSlot().placeholder
    this._renderBreadcrumb()
    this._loadSlotCandidates("")
    this.inputTarget.focus()
  }

  _loadSlotCandidates(query) {
    const slot = this._activeSlot()
    if (slot.source === "folders") {
      this._loadFolders(query)
    } else {
      this._scheduleCaptureSearch(query, slot.types)
    }
  }

  _scheduleCaptureSearch(query, types) {
    clearTimeout(this.debounceTimer)
    if (query.length < 2) {
      this._abort()
      this.capture.candidates = []
      this.capture.loading = false
      this._render()
      return
    }
    this.capture.loading = true
    this._render()
    this.debounceTimer = setTimeout(() => this._runCaptureSearch(query, types), 300)
  }

  async _runCaptureSearch(query, types) {
    this._abort()
    const controller = new AbortController()
    this.abortController = controller
    const seq = ++this.searchSeq

    try {
      const response = await fetch(`/search?q=${encodeURIComponent(query)}&types=${encodeURIComponent(types)}`, {
        headers: { "Accept": "application/json" },
        signal: controller.signal
      })
      const data = await response.json()
      if (seq !== this.searchSeq || !this.capture) return
      this.capture.candidates = (data.results || []).map(r => ({ id: r.id, title: r.title, subtitle: r.subtitle, icon: r.icon }))
      this.capture.loading = false
      this.selectedIndex = 0
      this._render()
    } catch (error) {
      if (error.name === "AbortError" || seq !== this.searchSeq || !this.capture) return
      this.capture.candidates = []
      this.capture.loading = false
      this._render()
    }
  }

  // Folders are a small per-account set: fetch once on slot entry, filter locally.
  async _loadFolders(query) {
    const cap = this.capture
    if (!cap.folderCache) {
      cap.loading = true
      this._render()
      try {
        const emailId = cap.values.email.id
        const response = await fetch(`/email_messages/${emailId}/folders`, { headers: { "Accept": "application/json" } })
        const data = await response.json()
        cap.folderCache = (data.folders || []).map(f => ({ id: f.id, title: f.name, icon: "folder" }))
      } catch (error) {
        cap.folderCache = []
      }
      cap.loading = false
    }
    if (!this.capture) return
    const q = query.toLowerCase()
    cap.candidates = q ? cap.folderCache.filter(f => f.title.toLowerCase().includes(q)) : cap.folderCache
    this.selectedIndex = 0
    this._render()
  }

  _selectCandidate(candidate) {
    const cap = this.capture
    const slot = cap.def.slots[cap.slotIndex]
    cap.values[slot.key] = { id: candidate.id, title: candidate.title }

    this._abort()
    clearTimeout(this.debounceTimer)
    cap.slotIndex += 1
    cap.folderCache = null
    cap.candidates = []
    this.selectedIndex = 0
    this.inputTarget.value = ""

    if (cap.slotIndex >= cap.def.slots.length) {
      const { def, values } = cap
      this.capture = null
      def.run(values)
      this.close()
    } else {
      this._syncCaptureSlot()
    }
  }

  _popSlot() {
    const cap = this.capture
    this._abort()
    clearTimeout(this.debounceTimer)
    if (cap.slotIndex === 0) {
      this._exitCapture()
      return
    }
    cap.slotIndex -= 1
    delete cap.values[cap.def.slots[cap.slotIndex].key]
    cap.folderCache = null
    cap.candidates = []
    this.selectedIndex = 0
    this.inputTarget.value = ""
    this._syncCaptureSlot()
  }

  _exitCapture() {
    this._abort()
    clearTimeout(this.debounceTimer)
    this.capture = null
    this.serverResults = []
    this.searching = false
    this.selectedIndex = 0
    this.inputTarget.value = ""
    this.inputTarget.placeholder = this._searchPlaceholder
    this._renderBreadcrumb()
    this.inputTarget.focus()
    this._render()
  }

  _renderBreadcrumb() {
    if (!this.hasBreadcrumbTarget) return
    const cap = this.capture
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

  // --- Render ---

  _render() {
    const items = this._currentItems()
    if (this.selectedIndex >= items.length) this.selectedIndex = Math.max(0, items.length - 1)

    if (items.length === 0) {
      this.inputTarget.removeAttribute("aria-activedescendant")
      this.listTarget.innerHTML = this._isLoading() ? this._loadingRow() : this._emptyState()
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
    if (this._isLoading()) html += this._loadingRow()

    this.listTarget.innerHTML = html
    this.inputTarget.setAttribute("aria-activedescendant", `cp-item-${this.selectedIndex}`)
    this._scrollToSelected()
  }

  _isLoading() {
    return this.capture ? this.capture.loading : this.searching
  }

  // Rows carry NO per-element listeners: click + hover are delegated to the stable
  // list container (see the list target's data-action in command_palette.rb). The
  // list innerHTML is rebuilt on every search/render, and Stimulus re-binds
  // data-action on fresh nodes asynchronously — so a per-row click handler can be
  // missing in the gap between a re-render and that re-bind, dropping the click.
  // Delegating to the container (which is never replaced) keeps rows always clickable.
  // The data-cp-* hooks let _updateSelection recolor a row in place without a rebuild.
  _row(item, index) {
    const selected = index === this.selectedIndex
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
    if (this.capture) {
      const slot = this._activeSlot()
      const message = query
        ? `No matches for "${this._esc(query)}"`
        : (slot.source === "folders" ? "No folders" : "Type to search…")
      return `<div class="px-4 py-8 text-center text-xs text-gray-400">${message}</div>`
    }
    const message = query ? `No results for "${this._esc(query)}"` : "No matching commands"
    return `<div class="px-4 py-8 text-center text-xs text-gray-400">${message}</div>`
  }

  _scrollToSelected() {
    requestAnimationFrame(() => {
      const btn = this.listTarget.querySelector(`[data-index="${this.selectedIndex}"]`)
      if (btn) btn.scrollIntoView({ block: "nearest" })
    })
  }

  // Delegated from the list container: resolve the clicked row from the event path.
  selectItem(event) {
    const row = event.target.closest("[data-index]")
    if (!row) return
    const index = parseInt(row.dataset.index)
    const item = this._currentItems()[index]
    if (!item) return
    if (this.capture) {
      if (item.candidate) this._selectCandidate(item.candidate)
    } else {
      item.run()
      if (!item.keepOpen) this.close()
    }
  }

  // Delegated hover (mouseover bubbles; mouseenter does not). Repaint the selection
  // in place instead of rebuilding the list — rebuilding on hover is what made rows
  // briefly unclickable. Bail when the row is already selected to avoid churn.
  hoverItem(event) {
    if (this._keyboardNav) return
    const row = event.target.closest("[data-index]")
    if (!row) return
    const index = parseInt(row.dataset.index)
    if (index === this.selectedIndex) return
    this.selectedIndex = index
    this._updateSelection()
  }

  // Move the selection highlight without touching the list's DOM nodes, keeping the
  // delegated handlers and every row's identity stable. Used for hover + arrow keys.
  _updateSelection() {
    const rows = this.listTarget.querySelectorAll("[data-index]")
    if (!rows.length) return
    rows.forEach(row => {
      const selected = parseInt(row.dataset.index) === this.selectedIndex
      row.setAttribute("aria-selected", selected ? "true" : "false")
      row.classList.toggle("bg-accent-50", selected)
      row.classList.toggle("hover:bg-gray-100", !selected)
      this._recolor(row.querySelector("[data-cp-icon]"), selected, ["text-accent-600"], ["text-gray-400"])
      this._recolor(row.querySelector("[data-cp-title]"), selected, ["text-accent-700", "font-medium"], ["text-gray-700"])
      this._recolor(row.querySelector("[data-cp-sub]"), selected, ["text-accent-600/80"], ["text-gray-500"])
    })
    this.inputTarget.setAttribute("aria-activedescendant", `cp-item-${this.selectedIndex}`)
  }

  _recolor(el, selected, onClasses, offClasses) {
    if (!el) return
    onClasses.forEach(c => el.classList.toggle(c, selected))
    offClasses.forEach(c => el.classList.toggle(c, !selected))
  }

  _handleMouseMove() {
    this._keyboardNav = false
  }

  // --- Helpers ---

  _esc(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }

  _truncate(str, max) {
    return str.length > max ? `${str.slice(0, max - 1).trimEnd()}…` : str
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

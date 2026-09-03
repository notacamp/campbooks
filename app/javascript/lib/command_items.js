// Shared command engine for the Cmd+K command palette AND the Scout overlay.
//
// The item model (catalog / composite / current-email / calendar commands +
// server search results), the client-side filtering, the keyboard navigation,
// the debounced search, and the composite-capture state machine all live here so
// the classic palette (command_palette_controller.js) and the Scout overlay
// (scout_overlay_controller.js) never fork. Each controller is a thin "host"
// that owns only its own DOM: how a row is drawn, the breadcrumb, and the input.
//
// The engine holds the state (selection, results, capture) and calls back into
// the host for the few DOM touchpoints (see the host interface in the CommandEngine
// constructor). This keeps the palette's exact behaviour — including the in-place
// selection repaint that keeps rows clickable — while letting the overlay reuse
// every bit of the logic under its own visual skin.

export const ICONS = {
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

export const SPINNER = '<svg class="w-3.5 h-3.5 animate-spin text-gray-400" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path></svg>'

export class CommandEngine {
  // host is the controller. Required host methods:
  //   rawQuery() -> string            the input's current value
  //   config() -> { context, messageId, subject, folders, commands }
  //   leadItems() -> [item]           extra items shown first in browse mode
  //   post(url, params) / visit(url) / close()
  //   render()                        full list re-render (reads engine state)
  //   updateSelection()               in-place selection repaint (no rebuild)
  //   scrollToSelected() / renderBreadcrumb()
  //   setInputValue(v) / setPlaceholder(p) / focusInput() / searchPlaceholder()
  //   onCommandEnter(query) -> bool   optional; handle Cmd/Ctrl+Enter (overlay ask)
  constructor(host) {
    this.host = host
    this.reset()
  }

  reset() {
    this.selectedIndex = 0
    this.keyboardNav = false
    this.serverResults = []
    this.searching = false
    this.searchSeq = 0
    clearTimeout(this.debounceTimer)
    this.debounceTimer = null
    this._abort()
    this.capture = null
  }

  // --- Commands (instant, client-side) ---

  _catalogCommands() {
    const cfg = this.host.config()
    const source = cfg.commands && cfg.commands.length ? cfg.commands : this._fallbackCommands()
    return source.map(c => ({
      name: c.name,
      category: c.category,
      icon: c.icon,
      keywords: c.keywords,
      action: c.method === "post" ? () => this.host.post(c.url, {}) : () => this.host.visit(c.url)
    }))
  }

  _fallbackCommands() {
    return [
      { name: "Inbox", category: "Navigate", icon: "mail", url: "/email_messages" },
      { name: "Scout AI Chat", category: "Navigate", icon: "sparkles", url: "/scout" },
      { name: "Files", category: "Navigate", icon: "folder", url: "/files" },
      { name: "Settings", category: "Navigate", icon: "cog", url: "/settings" },
      { name: "Start new email", category: "Actions", icon: "pen", url: "/email_messages/new" }
    ]
  }

  // Composite (parameterized) commands: pick an email/tag/folder inline, then run.
  _compositeDefs() {
    const post = (url, params) => this.host.post(url, params)
    return [
      {
        id: "move",
        label: "Move email to folder",
        icon: "folder",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to move…" },
          { key: "folder", label: "Folder", source: "folders", icon: "folder", placeholder: "Move to which folder…" }
        ],
        run: v => post("/email_messages/bulk", { tool: "move_to_folder", "email_ids[]": v.email.id, folder_id: v.folder.id })
      },
      {
        id: "tag",
        label: "Tag email",
        icon: "tag",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to tag…" },
          { key: "tag", label: "Tag", source: "search", types: "tags", icon: "tag", placeholder: "Pick a tag…" }
        ],
        run: v => post(`/email_messages/${v.email.id}/tool`, { tool: "add_tag", "args[tag_name]": v.tag.title })
      },
      {
        id: "archive",
        label: "Archive email",
        icon: "archive",
        slots: [
          { key: "email", label: "Email", source: "search", types: "emails", icon: "mail", placeholder: "Search the email to archive…" }
        ],
        run: v => post(`/email_messages/${v.email.id}/tool`, { tool: "archive" })
      }
    ]
  }

  _compositeCommands() {
    return this._compositeDefs().map(def => ({
      name: `${def.label}…`,
      category: "Commands",
      icon: def.icon,
      keywords: def.id,
      keepOpen: true, // selecting enters capture mode rather than closing
      action: () => this._enterCapture(def)
    }))
  }

  // Per-message email actions — they operate on the email currently open, so they
  // sit under a "Current email" group with the email's subject as subtitle.
  _contextCommands() {
    const cfg = this.host.config()
    if (cfg.context === "calendar") return this._calendarContextCommands()

    const commands = []
    if (cfg.context !== "email-show" || !cfg.messageId) return commands

    const id = cfg.messageId
    const group = "Current email"
    const subject = cfg.subject ? this._truncate(cfg.subject, 64) : null
    const post = (url, params) => this.host.post(url, params)
    const add = (name, icon, action, subtitle = null) => commands.push({ name, category: group, subtitle, icon, action })

    add("Archive", "archive", () => post(`/email_messages/${id}/tool`, { tool: "archive" }))
    add("Reply", "reply", () => post(`/email_messages/${id}/compose`, { mode: "reply" }))
    add("Reply all", "reply-all", () => post(`/email_messages/${id}/compose`, { mode: "reply_all" }))
    add("Forward", "forward", () => post(`/email_messages/${id}/compose`, { mode: "forward" }))
    add("Create calendar event", "calendar", () => post(`/email_messages/${id}/tool`, { tool: "create_calendar_event" }), subject)
    add("Create task from email", "check", () => post(`/email_messages/${id}/tool`, { tool: "create_task_from_email" }), subject)
    add("Dismiss AI todo", "check", () => post(`/email_messages/${id}/dismiss_todo`, { _method: "patch" }))
    add("Star sender", "star", () => post(`/email_messages/${id}/tool`, { tool: "star_sender" }), subject)
    add("Block sender", "ban", () => post(`/email_messages/${id}/tool`, { tool: "block_sender" }), subject)

    for (const f of cfg.folders || []) {
      if (!f.id) continue
      add(`Move to ${f.name}`, "folder", () => post("/email_messages/bulk", { tool: "move_to_folder", "email_ids[]": id, folder_id: f.id }), subject)
    }

    return commands
  }

  _calendarContextCommands() {
    const href = (sel) => document.querySelector(sel)?.getAttribute("href")
    const next = href("[data-calendar-next]")
    const prev = href("[data-calendar-prev]")
    const commands = []
    if (next) commands.push({ name: "Calendar: next period", category: "Calendar", icon: "calendar", action: () => this.host.visit(next) })
    if (prev) commands.push({ name: "Calendar: previous period", category: "Calendar", icon: "calendar", action: () => this.host.visit(prev) })
    return commands
  }

  // --- Merged item model ---

  _commandItems() {
    const query = this.host.rawQuery().toLowerCase().trim()
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
      run: () => this.host.visit(r.url)
    }))
  }

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

  currentItems() {
    if (this.capture) return this._captureItems()
    const lead = (this.host.leadItems && this.host.leadItems()) || []
    return [...lead, ...this._commandItems(), ...this._resultItems()]
  }

  isLoading() {
    return this.capture ? this.capture.loading : this.searching
  }

  // --- Keyboard (dialog open) ---

  onKeydown(event) {
    const items = this.currentItems()
    switch (event.key) {
      case "Escape":
        event.preventDefault()
        if (this.capture) this._exitCapture()
        else this.host.close()
        break
      case "Backspace":
        if (this.capture && this.host.rawQuery() === "") {
          event.preventDefault()
          this._popSlot()
        }
        break
      case "ArrowDown":
        event.preventDefault()
        this.keyboardNav = true
        this.selectedIndex = Math.min(this.selectedIndex + 1, items.length - 1)
        this.host.updateSelection()
        this.host.scrollToSelected()
        break
      case "ArrowUp":
        event.preventDefault()
        this.keyboardNav = true
        this.selectedIndex = Math.max(this.selectedIndex - 1, 0)
        this.host.updateSelection()
        this.host.scrollToSelected()
        break
      case "Enter": {
        event.preventDefault()
        // Cmd/Ctrl+Enter lets a host override Enter (the overlay asks Scout).
        if ((event.metaKey || event.ctrlKey) && this.host.onCommandEnter && this.host.onCommandEnter(this.host.rawQuery())) break
        const item = items[this.selectedIndex]
        if (!item) break
        this._activate(item)
        break
      }
    }
  }

  _activate(item) {
    if (this.capture) {
      if (item.candidate) this._selectCandidate(item.candidate)
    } else {
      item.run()
      if (!item.keepOpen) this.host.close()
    }
  }

  // --- Search (input handler) ---

  onInput() {
    this.selectedIndex = 0
    const query = this.host.rawQuery().trim()
    if (this.capture) {
      this._loadSlotCandidates(query)
    } else {
      this._scheduleSearch(query)
      this.host.render()
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
      if (seq !== this.searchSeq || this.capture) return
      this.serverResults = data.results || []
      this.searching = false
      this.host.render()
    } catch (error) {
      if (error.name === "AbortError" || seq !== this.searchSeq || this.capture) return
      this.serverResults = []
      this.searching = false
      this.host.render()
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
    this.host.setInputValue("")
    this._syncCaptureSlot()
  }

  _activeSlot() {
    return this.capture.def.slots[this.capture.slotIndex]
  }

  _syncCaptureSlot() {
    this.host.setPlaceholder(this._activeSlot().placeholder)
    this.host.renderBreadcrumb()
    this._loadSlotCandidates("")
    this.host.focusInput()
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
      this.host.render()
      return
    }
    this.capture.loading = true
    this.host.render()
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
      this.host.render()
    } catch (error) {
      if (error.name === "AbortError" || seq !== this.searchSeq || !this.capture) return
      this.capture.candidates = []
      this.capture.loading = false
      this.host.render()
    }
  }

  async _loadFolders(query) {
    const cap = this.capture
    if (!cap.folderCache) {
      cap.loading = true
      this.host.render()
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
    this.host.render()
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
    this.host.setInputValue("")

    if (cap.slotIndex >= cap.def.slots.length) {
      const { def, values } = cap
      this.capture = null
      def.run(values)
      this.host.close()
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
    this.host.setInputValue("")
    this._syncCaptureSlot()
  }

  _exitCapture() {
    this._abort()
    clearTimeout(this.debounceTimer)
    this.capture = null
    this.serverResults = []
    this.searching = false
    this.selectedIndex = 0
    this.host.setInputValue("")
    this.host.setPlaceholder(this.host.searchPlaceholder())
    this.host.renderBreadcrumb()
    this.host.focusInput()
    this.host.render()
  }

  // --- Selection (delegated from host click/hover) ---

  selectByIndex(index) {
    const item = this.currentItems()[index]
    if (!item) return
    this._activate(item)
  }

  hoverByIndex(index) {
    if (this.keyboardNav) return
    if (index === this.selectedIndex) return
    this.selectedIndex = index
    this.host.updateSelection()
  }

  clampSelection(length) {
    if (this.selectedIndex >= length) this.selectedIndex = Math.max(0, length - 1)
  }

  // --- Helpers ---

  _truncate(str, max) {
    return str.length > max ? `${str.slice(0, max - 1).trimEnd()}…` : str
  }
}

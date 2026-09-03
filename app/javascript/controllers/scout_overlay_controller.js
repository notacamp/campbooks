import { Controller } from "@hotwired/stimulus"
import { CommandEngine, ICONS, SPINNER } from "lib/command_items"

// The global Scout overlay (bold layout). One <dialog>, one input, three answers:
//   browse/idle    — Scout suggestions + Recent threads + the command catalog
//   command/find   — live results (commands, search) with an "Ask Scout" lead row
//   conversation   — the Scout chat, streamed in via the same turbo targets the
//                    reply job broadcasts to (agent_messages_list / agent_typing)
//
// The command/find half reuses the shared CommandEngine (lib/command_items.js),
// so it never forks from the classic palette. The body (idle/conversation) is a
// turbo-frame loaded lazily from /scout/overlay on first open; the single input
// moves between the head (browse) and the foot (conversation).
const OVERLAY_SRC = "/scout/overlay"

// Heuristic: does the text read like a question/instruction for Scout (vs a
// command or a name to find)? Ends with "?", ≥3 words, or a Scout-y opener.
const ASK_OPENERS = /^(ask|draft|write|find|show|what|which|who|when|why|how|remind|summari[sz]e|explain|tell|help|can you|could you)\b/i

export default class extends Controller {
  static targets = [
    "dialog", "input", "head", "headSlot", "headTitle", "foot", "footSlot",
    "frame", "bodyScroll", "recentCount", "list", "idle"
  ]

  connect() {
    this.engine = new CommandEngine(this._host())
    this.mode = "browse"
    this.aiAvailable = false
    this.recentCount = 0
    this.threadId = null
    this.pendingQuestion = null

    this.headPlaceholder = this.hasInputTarget ? this.inputTarget.getAttribute("placeholder") : ""
    this.footPlaceholder = this.hasInputTarget ? (this.inputTarget.dataset.followupPlaceholder || this.headPlaceholder) : ""

    this.boundKeydown = this._handleKeydown.bind(this)
    this.boundDialogClick = this._handleDialogClick.bind(this)
    this.boundChipClick = this._handleChipClick.bind(this)
    this.boundMouseMove = () => { this.engine.keyboardNav = false }
    document.addEventListener("keydown", this.boundKeydown)
    if (this.hasDialogTarget) this.dialogTarget.addEventListener("click", this.boundChipClick)

    // Keep the conversation pinned to the newest message as replies stream in.
    this.observer = new MutationObserver(() => this._scrollConversation())
  }

  disconnect() {
    document.removeEventListener("keydown", this.boundKeydown)
    if (this.hasDialogTarget) this.dialogTarget.removeEventListener("click", this.boundChipClick)
    this.observer?.disconnect()
    this.engine.reset()
  }

  // ── Open / close ──────────────────────────────────────────────────────────

  open(event) {
    if (event) event.preventDefault()
    if (this.dialogTarget.open) return
    this.dialogTarget.showModal()
    this.dialogTarget.addEventListener("mousemove", this.boundMouseMove, true)
    // Lazy-load the body the first time; keep it across reopens on the same page.
    if (!this.frameTarget.getAttribute("src")) this._loadFrame(OVERLAY_SRC)
    this._focusInput()
    this.input()
  }

  // Opening straight from the docked bar by typing: seed the first character.
  openFromKey(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return
    if (!event.key || event.key.length !== 1) return
    event.preventDefault()
    this.open()
    this.inputTarget.value = event.key
    this.input()
  }

  close() {
    if (!this.dialogTarget.open) return
    this.dialogTarget.close()
  }

  // Fired by the dialog's close event (Esc / backdrop / close()). Browse mode:
  // forget the half-typed query so reopening on the same page shows idle again.
  // Conversation mode: leave the thread + foot input in place, so reopening on
  // the same page shows the same conversation (a full page load resets to idle).
  onClose() {
    this.dialogTarget.removeEventListener("mousemove", this.boundMouseMove, true)
    this.engine.reset()
    if (this.mode !== "conversation") {
      if (this.hasInputTarget) this.inputTarget.value = ""
      if (this.hasIdleTarget) this.idleTarget.hidden = false
    }
  }

  _handleDialogClick(event) {
    if (event.target === this.dialogTarget) this.close()
  }

  // Suggestion / follow-up chips (data-chat-input-text-param) fill the input and ask.
  _handleChipClick(event) {
    const chip = event.target.closest("[data-chat-input-text-param]")
    if (!chip) return
    event.preventDefault()
    const text = chip.dataset.chatInputTextParam
    if (!text) return
    this.inputTarget.value = text
    this.askScout(text)
  }

  // ── Keyboard ──────────────────────────────────────────────────────────────

  _handleKeydown(event) {
    if (!this.dialogTarget.open) {
      if ((event.metaKey || event.ctrlKey) && event.key === "k") {
        event.preventDefault()
        this.open()
      }
      return
    }
    if (this.mode === "conversation") {
      this._conversationKeydown(event)
    } else {
      this.engine.onKeydown(event)
    }
  }

  _conversationKeydown(event) {
    if (event.key === "Escape") {
      event.preventDefault()
      this.close()
    } else if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault()
      this.askScout(this.inputTarget.value)
    }
  }

  // ── Input (browse) ──────────────────────────────────────────────────────────

  input() {
    if (this.mode === "conversation") return
    const empty = this.inputTarget.value.trim() === ""
    if (this.hasIdleTarget) this.idleTarget.hidden = !empty
    // engine.onInput() schedules search + re-renders the list through _render().
    this.engine.onInput()
  }

  // Delegated row click / hover from the list container.
  selectItem(event) {
    const row = event.target.closest("[data-index]")
    if (!row) return
    this.engine.selectByIndex(parseInt(row.dataset.index))
  }

  hoverItem(event) {
    const row = event.target.closest("[data-index]")
    if (!row) return
    this.engine.hoverByIndex(parseInt(row.dataset.index))
  }

  // ── Ask Scout ───────────────────────────────────────────────────────────────

  // Fill from a chip / the input, or ask the current text. Enters a conversation
  // (loading the current thread's shell first if we're still browsing), then posts.
  askScout(text) {
    text = (text || this.inputTarget.value).trim()
    if (!text || !this.aiAvailable) return
    this.inputTarget.value = ""
    if (this.mode === "conversation" && this.threadId) {
      this._postMessage(this.threadId, text)
    } else {
      this.pendingQuestion = text
      this._loadFrame(`${OVERLAY_SRC}?current=1`)
    }
  }

  _postMessage(threadId, content) {
    const url = threadId ? `/scout/threads/${threadId}/messages` : "/scout"
    this._post(url, { content })
  }

  // ── Recent threads ──────────────────────────────────────────────────────────

  openThread(event) {
    if (event) event.preventDefault()
    const id = event.params?.threadId || event.currentTarget?.dataset?.scoutOverlayThreadIdParam
    if (!id) return
    this._loadFrame(`${OVERLAY_SRC}?thread_id=${encodeURIComponent(id)}`)
  }

  // "Recent · N" in the foot returns to browse/idle (which lists recent threads).
  toggleRecent(event) {
    if (event) event.preventDefault()
    this._loadFrame(OVERLAY_SRC)
  }

  // ── Frame lifecycle ─────────────────────────────────────────────────────────

  _loadFrame(url) {
    if (!this.hasFrameTarget) return
    if (this.frameTarget.getAttribute("src") === url) this.frameTarget.reload()
    else this.frameTarget.setAttribute("src", url)
  }

  // turbo:frame-load — the body was (re)loaded; read its mode + wire it up.
  bodyLoaded() {
    const meta = this.frameTarget.querySelector("[data-scout-overlay-mode]")
    const mode = meta?.dataset?.scoutOverlayMode || "idle"
    this.aiAvailable = meta?.dataset?.aiAvailable === "true"
    this.recentCount = parseInt(meta?.dataset?.recentCount || "0")

    if (mode === "conversation") {
      this.threadId = meta.dataset.threadId
      this._enterConversationMode(meta.dataset.threadTitle)
      this._scrollConversation()
      if (this.hasBodyScrollTarget) this.observer.observe(this.bodyScrollTarget, { childList: true, subtree: true })
      // A question was queued while browsing → send it now.
      if (this.pendingQuestion) {
        const q = this.pendingQuestion
        this.pendingQuestion = null
        this._postMessage(this.threadId, q)
      }
    } else {
      this.observer.disconnect()
      this._enterBrowseMode()
      this.input()
    }
    this._focusInput()
  }

  // ── Mode transitions (move the one input between head and foot) ──────────────

  _enterBrowseMode() {
    this.mode = "browse"
    if (this.hasHeadSlotTarget && this.hasInputTarget && this.inputTarget.parentElement !== this.headSlotTarget) {
      this.headSlotTarget.appendChild(this.inputTarget)
    }
    if (this.hasInputTarget) this.inputTarget.placeholder = this.headPlaceholder
    if (this.hasHeadSlotTarget) this.headSlotTarget.hidden = false
    if (this.hasHeadTitleTarget) this.headTitleTarget.hidden = true
    if (this.hasFootTarget) { this.footTarget.classList.add("hidden"); this.footTarget.classList.remove("flex") }
    this.threadId = null
  }

  _enterConversationMode(title) {
    this.mode = "conversation"
    if (this.hasFootSlotTarget && this.hasInputTarget && this.inputTarget.parentElement !== this.footSlotTarget) {
      this.footSlotTarget.appendChild(this.inputTarget)
    }
    if (this.hasInputTarget) { this.inputTarget.placeholder = this.footPlaceholder; this.inputTarget.value = "" }
    if (this.hasHeadSlotTarget) this.headSlotTarget.hidden = true
    if (this.hasHeadTitleTarget) { this.headTitleTarget.hidden = false; this.headTitleTarget.textContent = title || "Scout" }
    if (this.hasFootTarget) { this.footTarget.classList.remove("hidden"); this.footTarget.classList.add("flex") }
    if (this.hasRecentCountTarget) this.recentCountTarget.textContent = ` · ${this.recentCount}`
  }

  _scrollConversation() {
    if (this.hasBodyScrollTarget) this.bodyScrollTarget.scrollTop = this.bodyScrollTarget.scrollHeight
  }

  _focusInput() {
    if (this.hasInputTarget) requestAnimationFrame(() => this.inputTarget.focus())
  }

  // ── Host interface for the shared CommandEngine ──────────────────────────────

  _host() {
    return {
      rawQuery: () => this.hasInputTarget ? this.inputTarget.value : "",
      config: () => ({
        context: document.body.dataset.commandPaletteContextValue || "",
        messageId: document.body.dataset.commandPaletteMessageIdValue || "",
        subject: document.body.dataset.commandPaletteSubjectValue || "",
        folders: this._bodyJson("commandPaletteFoldersValue", []),
        commands: this._bodyJson("commandPaletteCommandsValue", [])
      }),
      // The "Ask Scout" lead row: shown when the text reads like a question and an
      // AI provider is configured. Cmd+Enter asks regardless (onCommandEnter).
      leadItems: () => this._askItem(),
      post: (url, params) => this._post(url, params),
      visit: (url) => Turbo.visit(url),
      close: () => this.close(),
      render: () => this._render(),
      updateSelection: () => this._updateSelection(),
      scrollToSelected: () => this._scrollToSelected(),
      renderBreadcrumb: () => {},
      setInputValue: (v) => { if (this.hasInputTarget) this.inputTarget.value = v },
      setPlaceholder: (p) => { if (this.hasInputTarget) this.inputTarget.placeholder = p },
      focusInput: () => this._focusInput(),
      searchPlaceholder: () => this.headPlaceholder,
      onCommandEnter: (query) => { if (this._asksLikeQuestion(query, true)) { this.askScout(query); return true } return false }
    }
  }

  _bodyJson(key, fallback) {
    try { return JSON.parse(document.body.dataset[key] || "null") ?? fallback } catch { return fallback }
  }

  _askItem() {
    if (this.engine.capture) return []
    const query = this.hasInputTarget ? this.inputTarget.value.trim() : ""
    if (!query || !this.aiAvailable || !this._asksLikeQuestion(query)) return []
    return [{
      group: "Scout",
      title: `${this.dialogTarget.dataset.askLabel || "Ask Scout"}: "${query}"`,
      icon: "sparkles",
      // keepOpen: asking enters the conversation in place; the engine must not
      // close the dialog after running this row.
      keepOpen: true,
      run: () => this.askScout(query)
    }]
  }

  _asksLikeQuestion(text, force = false) {
    const t = (text || "").trim()
    if (!t) return false
    if (force) return true
    return t.endsWith("?") || t.split(/\s+/).length >= 3 || ASK_OPENERS.test(t)
  }

  // ── List rendering (overlay skin over the shared item model) ─────────────────

  _render() {
    if (!this.hasListTarget) return
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
      html += `<div class="px-2 pt-3 pb-1 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">${this._esc(group)}</div>`
      for (const { item, index } of rows) html += this._row(item, index)
    }
    if (this.engine.isLoading()) html += this._loadingRow()

    this.listTarget.innerHTML = html
    this.inputTarget.setAttribute("aria-activedescendant", `so-item-${this.engine.selectedIndex}`)
    this._scrollToSelected()
  }

  _row(item, index) {
    const selected = index === this.engine.selectedIndex
    const subtitle = item.subtitle
      ? `<span data-so-sub class="block truncate text-[11.5px] ${selected ? "text-foreground/70" : "text-muted-foreground"}">${this._esc(item.subtitle)}</span>`
      : ""
    return `
      <button type="button" id="so-item-${index}" role="option" aria-selected="${selected}"
              class="flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors ${selected ? "bg-secondary" : "hover:bg-secondary/60"}"
              data-index="${index}">
        <span data-so-icon class="flex-shrink-0 ${selected ? "text-[color:var(--ember-solid)]" : "text-muted-foreground"}">${ICONS[item.icon] || ICONS.search}</span>
        <span class="min-w-0 flex-1">
          <span data-so-title class="block truncate text-[13.5px] ${selected ? "font-medium text-foreground" : "text-foreground/90"}">${this._esc(item.title)}</span>
          ${subtitle}
        </span>
      </button>`
  }

  _loadingRow() {
    return `<div class="flex items-center gap-2 px-3 py-3 text-[13px] text-muted-foreground">${SPINNER}<span>Searching…</span></div>`
  }

  _emptyState() {
    const query = this.hasInputTarget ? this.inputTarget.value.trim() : ""
    const message = query ? `No results for "${this._esc(query)}"` : "Type to search, or ask Scout a question."
    return `<div class="px-4 py-8 text-center text-[13px] text-muted-foreground">${message}</div>`
  }

  _updateSelection() {
    if (!this.hasListTarget) return
    const rows = this.listTarget.querySelectorAll("[data-index]")
    if (!rows.length) return
    rows.forEach(row => {
      const selected = parseInt(row.dataset.index) === this.engine.selectedIndex
      row.setAttribute("aria-selected", selected ? "true" : "false")
      row.classList.toggle("bg-secondary", selected)
      row.classList.toggle("hover:bg-secondary/60", !selected)
      this._recolor(row.querySelector("[data-so-icon]"), selected, ["text-[color:var(--ember-solid)]"], ["text-muted-foreground"])
      this._recolor(row.querySelector("[data-so-title]"), selected, ["font-medium", "text-foreground"], ["text-foreground/90"])
      this._recolor(row.querySelector("[data-so-sub]"), selected, ["text-foreground/70"], ["text-muted-foreground"])
    })
    this.inputTarget.setAttribute("aria-activedescendant", `so-item-${this.engine.selectedIndex}`)
  }

  _recolor(el, selected, onClasses, offClasses) {
    if (!el) return
    onClasses.forEach(c => el.classList.toggle(c, selected))
    offClasses.forEach(c => el.classList.toggle(c, !selected))
  }

  _scrollToSelected() {
    requestAnimationFrame(() => {
      const btn = this.listTarget.querySelector(`[data-index="${this.engine.selectedIndex}"]`)
      if (btn) btn.scrollIntoView({ block: "nearest" })
    })
  }

  // ── Helpers ─────────────────────────────────────────────────────────────────

  _esc(str) {
    const div = document.createElement("div")
    div.textContent = str
    return div.innerHTML
  }

  _post(url, params) {
    const body = new FormData()
    for (const [key, value] of Object.entries(params)) body.append(key, value)
    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    fetch(url, {
      method: "POST",
      headers: { "X-CSRF-Token": csrfToken, "Accept": "text/vnd.turbo-stream.html, text/html" },
      body
    }).then(response => {
      if (response.headers.get("Content-Type")?.includes("text/vnd.turbo-stream.html")) {
        return response.text().then(html => { if (html) Turbo.renderStreamMessage(html) })
      } else if (response.redirected) {
        Turbo.visit(response.url)
      } else {
        return response.text().then(html => { if (html) Turbo.renderStreamMessage(html) })
      }
    }).catch(() => {})
  }
}

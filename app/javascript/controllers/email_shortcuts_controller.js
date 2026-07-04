import { Controller } from "@hotwired/stimulus"
import { skimOverlayOpen } from "controllers/skim_utils"

const EDITABLE_SELECTOR = "input, textarea, select, [contenteditable], [role=textbox]"

export default class extends Controller {
  static values = {
    messageId: String,
    context: String
  }

  connect() {
    this.boundKeydown = this._keydown.bind(this)
    this.boundFrameLoad = this._onFrameLoad.bind(this)
    window.addEventListener("keydown", this.boundKeydown)
    // The email_detail frame wraps only the reading pane; navigating it leaves
    // the thread list untouched. After each in-frame navigation, take the open
    // email's id from the frame's [data-detail-context] (URL-independent — the
    // advance visit may not have pushed history yet) and move the row highlight.
    document.addEventListener("turbo:frame-load", this.boundFrameLoad)
    // Advance to the next thread whenever the open one's row is removed by any
    // archive/trash/snooze (keyboard, Scout, command palette, bulk).
    this.boundStreamRender = this._onStreamRender.bind(this)
    document.addEventListener("turbo:before-stream-render", this.boundStreamRender)
    // Upgrade inbox row clicks to in-frame navigation (capture: before Turbo).
    this.boundThreadClick = this._interceptThreadClick.bind(this)
    document.addEventListener("click", this.boundThreadClick, true)
    this._syncActiveRowFromUrl()
    this._syncMessageIdFromUrl()
  }

  disconnect() {
    window.removeEventListener("keydown", this.boundKeydown)
    document.removeEventListener("turbo:frame-load", this.boundFrameLoad)
    document.removeEventListener("turbo:before-stream-render", this.boundStreamRender)
    document.removeEventListener("click", this.boundThreadClick, true)
  }

  // Rewrites a thread-row click from the default full-page visit to an
  // email_detail frame navigation, so only the reading pane swaps and the list
  // keeps its DOM — and scroll position. Runs in the capture phase so the new
  // data attributes are in place before Turbo reads the link.
  _interceptThreadClick(event) {
    if (event.defaultPrevented) return
    if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return

    const link = event.target.closest("a[href]")
    if (!link || !link.closest('[data-inbox-pane="list"]')) return
    if (link.closest('[data-select-mode="on"]')) return // tap = select, not open

    const href = link.getAttribute("href") || ""
    if (!this._messageIdFromPath(href)) return
    // Intent links (?compose=follow_up) need a full visit: auto_draft_controller
    // reads the flag from the URL on page load.
    if (/[?&]compose=/.test(href)) return

    // In the List/Board layouts rows open the bottom-right drawer instead
    // (email_drawer_controller's own capture handler takes them).
    const layout = document.querySelector("[data-inbox-layout]")?.getAttribute("data-inbox-layout")
    if (layout === "list" || layout === "board") return

    const frame = document.getElementById("email_detail")
    if (!frame || !frame.dataset.detailNav) return
    // The search/index shell hides the reading pane below lg — full visit there.
    if (frame.dataset.detailNav === "when-visible" && frame.getClientRects().length === 0) return

    link.dataset.turboFrame = "email_detail"
    link.dataset.turboAction = "advance"

    // Optimistic highlight — the response no longer re-renders the list.
    const row = link.closest('[id*="thread_item"]')
    if (row && !row.hasAttribute("data-active")) {
      const current = document.querySelector('[id*="thread_item"][data-active]')
      if (current) this._deactivateRow(current)
      this._activateRow(row)
    }
  }

  _onFrameLoad(event) {
    if (!event.target || event.target.id !== "email_detail") return
    const context = event.target.querySelector("[data-detail-context]")
    const id = (context && context.dataset.messageId) || this._messageIdFromPath(window.location.pathname)
    if (!id) return
    this.messageIdValue = id
    this._syncActiveRowToId(id)
  }

  // When a Turbo Stream is about to remove the currently-open thread's row (an
  // archive/trash/snooze from any surface), advance the detail pane to the next
  // thread. The sibling is captured before the removal, while it's still there.
  // A replace/update of the open row (e.g. the mark-read broadcast fired by
  // opening it) re-renders it without the highlight — re-apply it after render.
  _onStreamRender(event) {
    const stream = event.target
    if (!stream || stream.tagName !== "TURBO-STREAM") return
    const action = stream.getAttribute("action")
    const target = stream.getAttribute("target") || ""

    if (action === "remove") {
      const removed = document.getElementById(target)
      if (!removed || !removed.hasAttribute("data-active")) return

      const nextHref = this._nextThreadHref()
      if (nextHref) setTimeout(() => this._visitThread(nextHref), 0)
    } else if ((action === "replace" || action === "update") && target.includes("thread_item")) {
      setTimeout(() => {
        const id = this.messageIdValue || this._messageIdFromPath(window.location.pathname)
        if (id) this._syncActiveRowToId(id)
      }, 0)
    }
  }

  // The email_detail turbo frame reloads on navigation, but the body-level
  // message-id value only renders on a full page load — so keep it in sync with
  // the URL, or reply/archive/forward would act on the previously-open email.
  _syncMessageIdFromUrl() {
    const id = this._messageIdFromPath(window.location.pathname)
    if (id) this.messageIdValue = id
  }

  // --- Key handler ---

  _keydown(event) {
    if (this._shouldIgnore(event)) return

    const { key, shiftKey } = event

    switch (true) {
      case key === "e": event.preventDefault(); this._archive(); break
      case key === "#": event.preventDefault(); this._delete(); break
      case key === "r": event.preventDefault(); this._reply(); break
      case key === "a": event.preventDefault(); this._replyAll(); break
      case key === "f": event.preventDefault(); this._forward(); break
      case key === "x": event.preventDefault(); this._toggleSelect(); break
      case key === "c": event.preventDefault(); this._compose(); break
      case key === "?" || (shiftKey && key === "/"): event.preventDefault(); this._showHelp(); break
      case shiftKey && key === "I": event.preventDefault(); this._markRead(); break
      case shiftKey && key === "U": event.preventDefault(); this._markUnread(); break
      case key === "Escape": this._handleEscape(); break
      case key === "ArrowDown": event.preventDefault(); this._navigateThread(1); break
      case key === "ArrowUp": event.preventDefault(); this._navigateThread(-1); break
      case key === "ArrowLeft": event.preventDefault(); this._swipeActive("left"); break
      case key === "ArrowRight": event.preventDefault(); this._swipeActive("right"); break
    }
  }

  // ArrowLeft/ArrowRight fire the active row's swipe actions (archive / trash) via
  // the same confirm + animation pipeline as a real swipe.
  _swipeActive(direction) {
    const row = document.querySelector('[data-active="true"]')
    if (!row) return
    const ctrl = this.application.getControllerForElementAndIdentifier(row, "swipe-actions")
    if (ctrl) ctrl.triggerStage(direction)
  }

  _shouldIgnore(event) {
    if (event.metaKey || event.ctrlKey || event.altKey) return true

    // Defer to the calendar's own keyboard controller on the calendar page, so
    // c/a/r/e/arrows drive the calendar instead of email actions.
    if (document.body.dataset.calendarKeys) return true

    // Respect the command palette
    const palette = document.querySelector(".command-palette-dialog[open]")
    if (palette) return true

    // Defer to the Skim overlay: it's a role="dialog" panel (not a native
    // <dialog open>), so its own shortcuts would otherwise also archive/reply on
    // the inbox behind it. Stay silent while it has the keyboard.
    if (skimOverlayOpen()) return true

    // Don't intercept typing in editable fields
    const el = document.activeElement
    if (el && (el.matches(EDITABLE_SELECTOR) || el.closest(EDITABLE_SELECTOR))) return true

    // Don't intercept in modals (except our own help modal)
    const modal = document.querySelector("dialog[open]:not(#keyboard-shortcuts-modal)")
    if (modal) return true

    // Defer to drawer instance when present (avoid double-firing)
    if (this.contextValue !== "drawer" && document.querySelector("[data-email-shortcuts-context-value=drawer]")) {
      return true
    }

    // Don't intercept when a dropdown menu is open
    if (document.querySelector("[data-email-selection-target$=Menu]:not(.hidden)")) return true

    return false
  }

  // --- Actions ---

  // With a multi-select active, act on the selection; otherwise act on the
  // currently-open thread (Gmail-style: "e" archives the email you're reading).
  // Advancing to the next thread is handled centrally in _onStreamRender.
  _archive() {
    if (this._hasSelection()) {
      this._bulkAction("archive")
    } else if (this._hasMessageId()) {
      this._toolAction("archive")
    }
  }

  _delete() {
    if (this._hasSelection()) {
      this._bulkAction("delete")
    } else if (this._hasMessageId()) {
      this._toolAction("trash")
    }
  }

  _nextThreadHref() {
    const rows = Array.from(document.querySelectorAll('[id*="thread_item"]'))
    const idx = rows.findIndex(row => row.hasAttribute("data-active"))
    if (idx === -1) return null
    const target = rows[idx + 1] || rows[idx - 1] // next, or previous if it was last
    const link = target && target.querySelector("a[href*='/email_messages/']")
    return link ? link.href : null
  }

  _visitThread(href) {
    const frame = document.getElementById("email_detail")
    if (!frame) { window.location = href; return }
    frame.src = href
    history.pushState(null, "", href)
  }

  _reply() {
    if (this._hasMessageId()) {
      this._composeAction("reply")
    }
  }

  _replyAll() {
    if (this._hasMessageId()) {
      this._composeAction("reply_all")
    }
  }

  _forward() {
    if (this._hasMessageId()) {
      this._composeAction("forward")
    }
  }

  _toggleSelect() {
    const row = document.querySelector('[data-active="true"]')
    if (!row) return

    const checkbox = row.querySelector('[data-email-selection-target="checkbox"]')
    if (checkbox) {
      checkbox.checked = !checkbox.checked
      checkbox.dispatchEvent(new Event("change", { bubbles: true }))
    }
  }

  // Honors the user's compose_default (body[data-compose-default]): the Desk
  // navigates to the full page; the Dock opens the sheet in place.
  _compose() {
    if (document.body.dataset.composeDefault === "dock") {
      const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
      fetch("/email_messages/compose_new", {
        method: "POST",
        headers: {
          "X-CSRF-Token": csrfToken,
          "Accept": "text/vnd.turbo-stream.html",
          "Content-Type": "application/x-www-form-urlencoded"
        },
        body: "mode=new_message"
      }).then(r => this._renderStreamResponse(r))
    } else {
      window.location = "/email_messages/new"
    }
  }

  // Wired on the topbar Compose button so a click follows the same preference
  // as the C shortcut.
  composeClick(event) {
    if (document.body.dataset.composeDefault !== "dock") return
    event.preventDefault()
    this._compose()
  }

  _showHelp() {
    const dialog = document.getElementById("keyboard-shortcuts-modal")
    if (dialog) dialog.showModal()
  }

  _markRead() {
    if (this._hasSelection()) {
      this._bulkAction("mark_read")
    }
  }

  _markUnread() {
    if (this._hasSelection()) {
      this._bulkAction("mark_unread")
    }
  }

  _navigateThread(direction) {
    const rows = Array.from(document.querySelectorAll('[id*="thread_item"]'))
    if (rows.length === 0) return

    const activeRow = rows.find(row => row.hasAttribute("data-active"))
    let currentIndex = activeRow ? rows.indexOf(activeRow) : (direction > 0 ? -1 : rows.length)

    const nextIndex = currentIndex + direction
    if (nextIndex < 0 || nextIndex >= rows.length) return

    const nextRow = rows[nextIndex]
    const link = nextRow.querySelector("a[href*='/email_messages/']")
    if (!link) return

    // Update thread list highlight
    if (activeRow) this._deactivateRow(activeRow)
    this._activateRow(nextRow)

    // Load detail pane via Turbo Frame
    const frame = document.getElementById("email_detail")
    if (frame) {
      frame.src = link.href
      history.pushState(null, "", link.href)
    }
  }

  // Mark the thread row matching the current URL as active. Covers the initial
  // page load, before any in-frame navigation has stamped messageIdValue. The
  // drawer instance is a single-message view with no list, so it opts out.
  _syncActiveRowFromUrl() {
    if (this.contextValue === "drawer") return

    const id = this._messageIdFromPath(window.location.pathname)
    if (id) this._syncActiveRowToId(id)
  }

  // Move the list highlight to the row whose link points at the given message.
  _syncActiveRowToId(id) {
    if (this.contextValue === "drawer") return

    const rows = Array.from(document.querySelectorAll('[id*="thread_item"]'))
    if (rows.length === 0) return

    const target = rows.find(row => {
      const link = row.querySelector("a[href*='/email_messages/']")
      const linkId = link && this._messageIdFromPath(link.getAttribute("href"))
      return linkId === id
    })
    if (!target) return

    const current = rows.find(row => row.hasAttribute("data-active"))
    if (current !== target) {
      if (current) this._deactivateRow(current)
      this._activateRow(target)
    }

    // With in-place navigation the previously clicked/focused row link stays in
    // the DOM and keeps focus — its focus ring would linger on the wrong row
    // (arrows/j-k move the selection without moving focus). Keep focus with the
    // open thread, but only when it's already parked on some other row link.
    const focused = document.activeElement
    if (focused && focused.closest("[id*='thread_item']") && !target.contains(focused)) {
      target.querySelector("a[href*='/email_messages/']")?.focus()
    }
  }

  // Mirrors _thread_row.html.erb's active/inactive classes (Swipeable wrapper
  // carries the bg, the row link carries the text accent).
  _activateRow(row) {
    row.setAttribute("data-active", "true")
    row.classList.add("bg-subtle")
    row.classList.remove("hover:bg-muted")
    const link = row.querySelector("a")
    if (link) link.classList.add("text-accent-700")
    // Opening marks the thread read — drop the unread affordances right away
    // (the ember dot and bold subject) instead of waiting for the broadcast.
    const dot = row.querySelector(".bg-ember")
    if (dot) dot.remove()
    const bold = row.querySelector(".font-bold")
    if (bold) bold.classList.remove("font-bold")
  }

  _deactivateRow(row) {
    row.removeAttribute("data-active")
    row.classList.remove("bg-subtle")
    row.classList.add("hover:bg-muted")
    const link = row.querySelector("a")
    if (link) link.classList.remove("text-accent-700")
  }

  _handleEscape() {
    // Default Escape behavior (clear selection) is handled by email-selection controller.
    // Close the help modal if it's open.
    const dialog = document.getElementById("keyboard-shortcuts-modal")
    if (dialog && dialog.open) {
      dialog.close()
    }
  }

  // --- Helpers ---

  // Extract an email message id from a path or href. Message ids are UUIDs (the
  // app uses uuid primary keys); a `\d+`-only match truncated a UUID like
  // "71f1ae2f-…" to its leading digits ("71"), so every keyboard action then hit
  // /email_messages/71/… → a 404 whose full HTML error PAGE, fed to
  // Turbo.renderStreamMessage, wiped the inbox's styles. Match a UUID (or a
  // numeric id) — mirrors the fix in email_drawer_controller.
  _messageIdFromPath(path) {
    return (String(path).match(/\/email_messages\/([0-9a-f-]{8,}|\d+)/i) || [])[1]
  }

  // Only render responses that are actually Turbo Streams — a successful action
  // or a handled 422 error-toast. An error PAGE (404/500 → a full HTML document)
  // must never reach Turbo.renderStreamMessage: it strips the current document's
  // styles and chrome. Guarding on content-type bounds the blast radius.
  _renderStreamResponse(response) {
    if (!(response.headers.get("content-type") || "").includes("turbo-stream")) return
    return response.text().then(html => { if (html) Turbo.renderStreamMessage(html) })
  }

  _hasMessageId() {
    return this.hasMessageIdValue && this.messageIdValue.length > 0
  }

  _hasSelection() {
    const controller = this._selectionController()
    return controller && controller.selected && controller.selected.size > 0
  }

  _bulkAction(tool) {
    const btn = document.querySelector(`[data-tool="${tool}"][data-action*="email-selection#bulkAction"]`)
    if (btn) {
      btn.click()
    } else {
      // Fallback: POST directly
      const controller = this._selectionController()
      if (!controller || controller.selected.size === 0) return

      const body = new FormData()
      body.append("tool", tool)
      controller.selected.forEach(id => body.append("email_ids[]", id))

      const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
      fetch("/email_messages/bulk", {
        method: "POST",
        headers: { "X-CSRF-Token": csrfToken, "Accept": "text/vnd.turbo-stream.html" },
        body
      }).then(r => this._renderStreamResponse(r))

      controller.clear()
    }
  }

  _toolAction(tool) {
    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    return fetch(`/email_messages/${this.messageIdValue}/tool`, {
      method: "POST",
      headers: {
        "X-CSRF-Token": csrfToken,
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: `tool=${encodeURIComponent(tool)}`
    }).then(r => this._renderStreamResponse(r))
  }

  _composeAction(mode) {
    const csrfToken = document.querySelector("meta[name='csrf-token']")?.content
    fetch(`/email_messages/${this.messageIdValue}/compose`, {
      method: "POST",
      headers: {
        "X-CSRF-Token": csrfToken,
        "Accept": "text/vnd.turbo-stream.html",
        "Content-Type": "application/x-www-form-urlencoded"
      },
      body: `mode=${encodeURIComponent(mode)}`
    }).then(r => this._renderStreamResponse(r))
  }

  _selectionController() {
    const el = document.querySelector("[data-controller~=email-selection]")
    if (!el) return null
    return this.application.getControllerForElementAndIdentifier(el, "email-selection")
  }
}

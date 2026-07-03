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
    this.boundFrameRender = this._onFrameRender.bind(this)
    window.addEventListener("keydown", this.boundKeydown)
    // The thread list lives inside the "email_detail" turbo frame, so in-frame
    // navigation reloads it and can drop the server-rendered highlight. Re-derive
    // the active row from the URL on load and after every frame render so the
    // row-dependent shortcuts (x/e/#/mark, arrows) always have an anchor.
    document.addEventListener("turbo:frame-render", this.boundFrameRender)
    // Advance to the next thread whenever the open one's row is removed by any
    // archive/trash/snooze (keyboard, Scout, command palette, bulk).
    this.boundStreamRender = this._onStreamRender.bind(this)
    document.addEventListener("turbo:before-stream-render", this.boundStreamRender)
    this._syncActiveRowFromUrl()
    this._syncMessageIdFromUrl()
  }

  disconnect() {
    window.removeEventListener("keydown", this.boundKeydown)
    document.removeEventListener("turbo:frame-render", this.boundFrameRender)
    document.removeEventListener("turbo:before-stream-render", this.boundStreamRender)
  }

  // When a Turbo Stream is about to remove the currently-open thread's row (an
  // archive/trash/snooze from any surface), advance the detail pane to the next
  // thread. The sibling is captured before the removal, while it's still there.
  _onStreamRender(event) {
    const stream = event.target
    if (!stream || stream.tagName !== "TURBO-STREAM" || stream.getAttribute("action") !== "remove") return

    const removed = document.getElementById(stream.getAttribute("target") || "")
    if (!removed || !removed.hasAttribute("data-active")) return

    const nextHref = this._nextThreadHref()
    if (nextHref) setTimeout(() => this._visitThread(nextHref), 0)
  }

  _onFrameRender(event) {
    if (event.target && event.target.id === "email_detail") {
      this._syncActiveRowFromUrl()
      this._syncMessageIdFromUrl()
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

  _compose() {
    window.location = "/email_messages/new"
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

  // Mark the thread row matching the current URL as active. Covers both initial
  // load and frame reloads where the server-rendered highlight was lost. The
  // drawer instance is a single-message view with no list, so it opts out.
  _syncActiveRowFromUrl() {
    if (this.contextValue === "drawer") return

    const id = this._messageIdFromPath(window.location.pathname)
    if (!id) return

    const rows = Array.from(document.querySelectorAll('[id*="thread_item"]'))
    if (rows.length === 0) return

    const target = rows.find(row => {
      const link = row.querySelector("a[href*='/email_messages/']")
      const linkId = link && this._messageIdFromPath(link.getAttribute("href"))
      return linkId === id
    })
    if (!target) return

    const current = rows.find(row => row.hasAttribute("data-active"))
    if (current === target) return
    if (current) this._deactivateRow(current)
    this._activateRow(target)
  }

  _activateRow(row) {
    row.setAttribute("data-active", "true")
    row.classList.add("bg-accent-50/50", "border-accent-500")
    row.classList.remove("hover:bg-gray-50/70", "border-transparent")
    const link = row.querySelector("a")
    if (link) link.classList.add("text-accent-700")
  }

  _deactivateRow(row) {
    row.removeAttribute("data-active")
    row.classList.remove("bg-accent-50/50", "border-accent-500")
    row.classList.add("hover:bg-gray-50/70", "border-transparent")
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

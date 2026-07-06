import { Controller } from "@hotwired/stimulus"

export default class extends Controller {
  connect() {
    this.mode = "scout" // "scout" or "manual"
    this._onKeydown = this._handleKeydown.bind(this)
    document.addEventListener("keydown", this._onKeydown)

    // Intercept "send" trigger word in chat form
    this._interceptSendTrigger()

    // Auto-focus chat input
    setTimeout(() => {
      document.getElementById("compose_chat_input")?.focus()
    }, 100)
  }

  _interceptSendTrigger() {
    // Use event delegation on the chat panel body (stable, never replaced)
    const panel = this.element.querySelector("[data-chat-panel-target='body']")
    if (!panel) return setTimeout(() => this._interceptSendTrigger(), 200)

    panel.addEventListener("submit", (event) => {
      const form = event.target.closest("form")
      if (!form || form.id !== "agent_chat_form") return
      const input = form.querySelector("textarea")
      const text = input?.value?.trim().toLowerCase()
      if (["send", "send it", "ok", "yes", "go ahead", "go", "done", "go for it"].includes(text)) {
        event.preventDefault()
        event.stopImmediatePropagation()
        input.value = ""
        this.sendEmail()
      }
    }, true)
  }

  disconnect() {
    document.removeEventListener("keydown", this._onKeydown)
  }

  // ── Mode toggle: Cmd+Shift+E ──────────────────────────────

  _handleKeydown(event) {
    if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key === "E") {
      event.preventDefault()
      this.toggleMode()
    }
  }

  toggleMode() {
    if (this.mode === "scout") {
      this.mode = "manual"
      this.element.classList.add("mode-manual")
      // Focus the first compose form field
      setTimeout(() => {
        this.element.querySelector("input[name='to_address']")?.querySelector("input")?.focus()
      }, 100)
    } else {
      this.mode = "scout"
      this.element.classList.remove("mode-manual")
      setTimeout(() => {
        document.getElementById("compose_chat_input")?.focus()
      }, 100)
    }
  }

  // ── Context-rail chip bridge ──────────────────────────────

  // Called by Scout suggestion chips in the Compose::ContextRail. Prefills the
  // chat input with the chip text, opens the panel if collapsed, and focuses.
  prefillChat(event) {
    const text = event.params?.text || event.currentTarget.dataset.composeChatTextParam
    if (!text) return

    // Open the desktop chat panel if it is currently collapsed.
    const panelEl = document.querySelector("[data-controller~='chat-panel']")
    if (panelEl && window.innerWidth >= 1024) {
      const ctrl = this.application.getControllerForElementAndIdentifier(panelEl, "chat-panel")
      if (ctrl && !ctrl.open) ctrl.toggle()
    }

    // Surface the Scout overlay on mobile.
    if (window.innerWidth < 1024) {
      const scoutEl = document.querySelector("[data-controller~='scout-mobile']")
      if (scoutEl) {
        const ctrl = this.application.getControllerForElementAndIdentifier(scoutEl, "scout-mobile")
        ctrl?.open()
      }
    }

    // Fill the input and enable the submit button.
    const input = document.getElementById("compose_chat_input")
    if (!input) return
    input.value = text
    input.dispatchEvent(new Event("input", { bubbles: true }))
    setTimeout(() => input.focus(), 150)
  }

  // ── Form-filling bridge ───────────────────────────────────

  setRecipients(event) {
    const to = event.currentTarget?.dataset?.composeChatToParam || event.params?.to
    const cc = event.currentTarget?.dataset?.composeChatCcParam || event.params?.cc

    if (to) this._fillPillInput("to_address", to)
    if (cc) this._fillPillInput("cc_address", cc)
  }

  setSubject(event) {
    const subject = event.currentTarget?.dataset?.composeChatSubjectParam || event.params?.subject
    if (!subject) return

    const input = this.element.querySelector("input[name='subject']")
    if (input) { input.value = subject; input.dispatchEvent(new Event("input", { bubbles: true })) }
  }

  setBody(event) {
    const body = event.currentTarget?.dataset?.composeChatBodyParam || event.params?.body
    if (!body) return

    // Find the TipTap editor controller and set content
    const editorEl = this.element.querySelector("[data-controller~='tiptap-editor']")
    if (!editorEl) return

    const tiptapController = this._getController(editorEl, "tiptap-editor")
    if (tiptapController?.setContent) {
      tiptapController.setContent(body)
    }
  }

  setFromAccount(event) {
    const accountId = event.currentTarget?.dataset?.composeChatAccountIdParam || event.params?.account_id
    if (!accountId) return

    const select = this.element.querySelector("select[name='email_account_id']")
    if (select) {
      select.value = accountId
      select.dispatchEvent(new Event("change", { bubbles: true }))
    }
  }

  // Called by <script> tags in broadcasts to execute auto_actions
  __executeAutoAction__(tool, args) {
    switch (tool) {
      case "set_recipients":
        if (args.to) this._fillPillInput("to_address", args.to)
        if (args.cc) this._fillPillInput("cc_address", args.cc)
        break
      case "set_subject":
        this._setInputValue("subject", args.subject)
        break
      case "set_body":
        this._setBodyContent(args.body)
        break
      case "select_account":
        this._setSelectValue("email_account_id", args.account_id)
        break
      case "set_signature":
        this._setSignatureById(args.signature_id)
        break
      case "send_email":
        this.sendEmail()
        break
    }
  }

  _setInputValue(name, value) {
    const input = this.element.querySelector(`input[name='${name}']`)
    if (input) { input.value = value; input.dispatchEvent(new Event("input", { bubbles: true })) }
  }

  _setSelectValue(name, value) {
    const select = this.element.querySelector(`select[name='${name}']`)
    if (select) { select.value = value; select.dispatchEvent(new Event("change", { bubbles: true })) }
  }

  _setBodyContent(body) {
    const editorEl = this.element.querySelector("[data-controller~='tiptap-editor']")
    if (!editorEl) return
    const tc = this.application.getControllerForElementAndIdentifier(editorEl, "tiptap-editor")
    if (tc?.setContent) tc.setContent(body)
  }

  // ── Signature management ─────────────────────────────────

  onFromAccountChange(event) {
    const accountId = event.target.value
    const sigSelect = this.element.querySelector("[data-compose-chat-target='signatureSelect']")
    if (!sigSelect) return

    const options = Array.from(sigSelect.options)

    // Hide signatures pinned to *other* accounts. Unassigned signatures, and the
    // global default, stay visible for every account (the default is the
    // app-wide fallback — mirrors Signature.default_for).
    let firstForAccount = null
    let defaultOption = null
    options.forEach(opt => {
      if (!opt.value) { opt.hidden = false; return } // "No signature" always visible
      const accountIds = (opt.dataset.accountIds || "").split(",").filter(Boolean)
      const isDefault = opt.dataset.default === "true"
      const pinnedElsewhere = accountIds.length > 0 && !accountIds.includes(accountId)
      opt.hidden = pinnedElsewhere && !isDefault
      if (accountIds.includes(accountId) && !firstForAccount) firstForAccount = opt
      if (isDefault && !defaultOption) defaultOption = opt
    })

    // Auto-pick this account's signature (mirrors Signature.default_for):
    // a signature assigned to the account wins; else the global default; else
    // the first still-visible signature; else none.
    const firstVisible = options.find(opt => opt.value && !opt.hidden)
    const pick = firstForAccount || defaultOption || firstVisible
    sigSelect.value = pick ? pick.value : ""
    this.selectSignature({ target: sigSelect })
  }

  selectSignature(event) {
    const select = event.target
    const selectedOption = select.options[select.selectedIndex]
    const signatureHtml = selectedOption?.dataset?.content || ""
    const signatureId = select.value

    this._updateSignaturePreview(signatureId, signatureHtml)
  }

  _updateSignaturePreview(signatureId, signatureHtml) {
    let preview = this.element.querySelector("#signature_preview")
    if (!preview) preview = document.getElementById("signature_preview")

    if (!signatureId || !signatureHtml) {
      if (preview) preview.innerHTML = ""
      return
    }

    if (preview) {
      preview.innerHTML = `<div class="text-[10px] text-gray-400 mb-1 uppercase tracking-wide">Signature</div><div class="text-xs text-gray-500 border border-gray-200 rounded-md p-2.5 bg-gray-50 max-h-20 overflow-y-auto">${signatureHtml}</div>`
    }
  }

  setSignature(event) {
    const signatureId = event.currentTarget?.dataset?.composeChatSignatureIdParam || event.params?.signature_id
    if (!signatureId) return
    this._setSignatureById(String(signatureId))
  }

  _setSignatureById(signatureId) {
    const sigSelect = this.element.querySelector("[data-compose-chat-target='signatureSelect']")
    if (!sigSelect) return
    sigSelect.value = signatureId
    this.selectSignature({ target: sigSelect })
  }

  sendEmail() {
    // Auto-apply any unapplied actions before sending
    const bodyBtn = this.element.querySelector("button[data-action='click->compose-chat#setBody']")
    if (bodyBtn) bodyBtn.click()

    const form = this.element.querySelector("form[data-controller~='compose-engine'], form[data-controller~='compose-area']")
    if (!form) return

    // Show sending state on the compose form
    const btn = form.querySelector("button[type=submit]")
    if (btn) {
      btn.setAttribute("disabled", "disabled")
      btn.innerHTML = `<svg class="w-3.5 h-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/>
      </svg> Sending…`
    }

    form.requestSubmit()
  }

  // ── Helpers ───────────────────────────────────────────────

  _fillPillInput(name, value) {
    // Add pills first (before hidden input gets set, to avoid dedup skip)
    const pillWrapper = this.element.querySelector(`[data-controller~='contact-pill-input']:has(input[name='${name}'])`)
    if (pillWrapper) {
      const controller = this._getController(pillWrapper, "contact-pill-input")
      if (controller?.addPill) {
        // Clear existing pills first
        const existing = pillWrapper.querySelectorAll('[data-email]')
        existing.forEach(el => el.remove())
        // Then add new ones
        value.split(",").forEach(email => {
          const trimmed = email.trim()
          if (trimmed) controller.addPill(trimmed, trimmed)
        })
      }
    }
  }

  _getController(element, identifier) {
    return this.application.getControllerForElementAndIdentifier(element, identifier)
  }
}

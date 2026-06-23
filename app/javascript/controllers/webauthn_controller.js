import { Controller } from "@hotwired/stimulus"

// Drives a WebAuthn ceremony (passkey registration or assertion) using the
// native browser API — no third-party library, so the login path has no runtime
// CDN dependency. Attach to a <form>; a type="button" trigger calls #start.
//
//   data-controller="webauthn"
//   data-webauthn-options-url-value="/.../options.json"   // GET → PublicKey options JSON
//   data-webauthn-mode-value="create"                     // "create" (enroll) | "get" (login)
//   data-webauthn-unsupported-message-value="..."
//   <input type="hidden" name="credential" data-webauthn-target="credential">
//   <p data-webauthn-target="error" hidden></p>
//   <button type="button" data-action="webauthn#start">…</button>
//
// On success the serialized credential is written into the hidden field and the
// form is submitted normally, so the server's redirect flow handles the result.
export default class extends Controller {
  static targets = ["credential", "error", "submit"]
  static values = {
    optionsUrl: String,
    mode: { type: String, default: "get" },
    unsupportedMessage: { type: String, default: "This browser doesn't support passkeys." }
  }

  async start() {
    this.clearError()

    if (!window.PublicKeyCredential) {
      this.showError(this.unsupportedMessageValue)
      return
    }

    this.disable(true)
    try {
      const options = await this.fetchOptions()
      const credential =
        this.modeValue === "create"
          ? await navigator.credentials.create({ publicKey: this.publicKeyForCreate(options) })
          : await navigator.credentials.get({ publicKey: this.publicKeyForGet(options) })

      this.credentialTarget.value = JSON.stringify(this.serialize(credential))
      this.submitForm()
    } catch (error) {
      // A user-cancelled prompt (NotAllowedError) shouldn't read as a failure.
      if (error?.name !== "NotAllowedError") this.showError(error?.message || this.unsupportedMessageValue)
      this.disable(false)
    }
  }

  async fetchOptions() {
    const response = await fetch(this.optionsUrlValue, {
      headers: { Accept: "application/json" },
      credentials: "same-origin"
    })
    if (!response.ok) throw new Error("Could not start the passkey ceremony.")
    return response.json()
  }

  // ── Option shaping (base64url → ArrayBuffer for the browser API) ──
  publicKeyForCreate(options) {
    return {
      ...options,
      challenge: this.decode(options.challenge),
      user: { ...options.user, id: this.decode(options.user.id) },
      excludeCredentials: (options.excludeCredentials || []).map((c) => ({ ...c, id: this.decode(c.id) }))
    }
  }

  publicKeyForGet(options) {
    return {
      ...options,
      challenge: this.decode(options.challenge),
      allowCredentials: (options.allowCredentials || []).map((c) => ({ ...c, id: this.decode(c.id) }))
    }
  }

  // ── Credential serialization (ArrayBuffer → base64url for the server) ──
  serialize(credential) {
    const response = credential.response
    const json = {
      type: credential.type,
      id: credential.id,
      rawId: this.encode(credential.rawId),
      response: { clientDataJSON: this.encode(response.clientDataJSON) }
    }
    if (this.modeValue === "create") {
      json.response.attestationObject = this.encode(response.attestationObject)
    } else {
      json.response.authenticatorData = this.encode(response.authenticatorData)
      json.response.signature = this.encode(response.signature)
      json.response.userHandle = response.userHandle ? this.encode(response.userHandle) : null
    }
    return json
  }

  // ── base64url helpers ──
  decode(value) {
    const padded = value.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(value.length / 4) * 4, "=")
    const binary = atob(padded)
    const bytes = new Uint8Array(binary.length)
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i)
    return bytes.buffer
  }

  encode(buffer) {
    const bytes = new Uint8Array(buffer)
    let binary = ""
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i])
    return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "")
  }

  submitForm() {
    const form = this.element
    if (form.requestSubmit) form.requestSubmit()
    else form.submit()
  }

  disable(state) {
    if (this.hasSubmitTarget) this.submitTarget.disabled = state
  }

  showError(message) {
    if (!this.hasErrorTarget) return
    this.errorTarget.textContent = message
    this.errorTarget.hidden = false
  }

  clearError() {
    if (this.hasErrorTarget) this.errorTarget.hidden = true
  }
}

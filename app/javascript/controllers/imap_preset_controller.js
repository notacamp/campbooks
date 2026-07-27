import { Controller } from "@hotwired/stimulus"

// Fills IMAP/SMTP server settings from a known-good preset when the user
// picks a common provider from the dropdown. Presets are inlined so nothing
// extra is fetched — the controller just copies values into the target inputs.
// Choosing "custom" clears the host fields so the user types their own.

const PRESETS = {
  icloud: {
    imapHost: "imap.mail.me.com", imapPort: "993", imapSecurity: "ssl",
    smtpHost: "smtp.mail.me.com", smtpPort: "587", smtpSecurity: "starttls"
  },
  fastmail: {
    imapHost: "imap.fastmail.com", imapPort: "993", imapSecurity: "ssl",
    smtpHost: "smtp.fastmail.com", smtpPort: "465", smtpSecurity: "ssl"
  },
  yahoo: {
    imapHost: "imap.mail.yahoo.com", imapPort: "993", imapSecurity: "ssl",
    smtpHost: "smtp.mail.yahoo.com", smtpPort: "465", smtpSecurity: "ssl"
  },
  gmx: {
    imapHost: "imap.gmx.com", imapPort: "993", imapSecurity: "ssl",
    smtpHost: "mail.gmx.com", smtpPort: "587", smtpSecurity: "starttls"
  },
  zoho: {
    imapHost: "imappro.zoho.com", imapPort: "993", imapSecurity: "ssl",
    smtpHost: "smtppro.zoho.com", smtpPort: "465", smtpSecurity: "ssl"
  }
}

export default class extends Controller {
  static targets = ["imapHost", "imapPort", "imapSecurity", "smtpHost", "smtpPort", "smtpSecurity"]

  apply(event) {
    const preset = PRESETS[event.target.value]

    if (preset) {
      this.imapHostTarget.value     = preset.imapHost
      this.imapPortTarget.value     = preset.imapPort
      this.imapSecurityTarget.value = preset.imapSecurity
      this.smtpHostTarget.value     = preset.smtpHost
      this.smtpPortTarget.value     = preset.smtpPort
      this.smtpSecurityTarget.value = preset.smtpSecurity
    } else {
      // "custom" — clear host fields so the user fills in their own server.
      this.imapHostTarget.value = ""
      this.smtpHostTarget.value = ""
    }
  }
}

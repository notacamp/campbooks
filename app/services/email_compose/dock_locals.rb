# frozen_string_literal: true

module EmailCompose
  # Builds the locals hash consumed by the `email_compose/dock` partial for a
  # blank new-message compose (no source message, no thread context).
  #
  # Shared by EmailComposeController and Reconciliations::BankTransactionsController
  # so both can open the Dock with identical structure. Keeps the two callers
  # from drifting when the dock partial's expected locals change.
  #
  # Usage:
  #   DockLocals.blank(user: current_user, to: "foo@example.com", subject: "Hi", body: "…")
  module DockLocals
    # Returns the locals hash expected by `email_compose/dock` for a blank compose.
    #
    # @param user     [User]   the current acting user (used to resolve signature + accounts)
    # @param to:      [String] pre-filled recipient address (editable in the composer)
    # @param subject: [String] pre-filled subject line
    # @param body:    [String] pre-filled body text (overrides signature when set)
    def self.blank(user:, to: "", subject: "", body: "")
      prefill_body = body.presence
      signature    = prefill_body.present? ? nil : Signature.default_for(user, nil)
      signatures   = user.signatures.ordered.includes(:email_accounts)
      accounts     = user.sendable_email_accounts.to_a

      {
        mode:              :new_message,
        message:           nil,
        draft:             nil,
        to:                to.to_s,
        cc:                "",
        bcc:               "",
        subject:           subject.to_s,
        body:              prefill_body.to_s,
        quoted_body:       "",
        signatures:        signatures,
        signature_id:      signature&.id,
        accounts:          accounts,
        attachment_entries: [],
        scout_draft:       nil
      }
    end
  end
end

# frozen_string_literal: true

module Emails
  # Records ONE SkimDecision for a cluster the user just acted on, so
  # Emails::SkimActionMemory can learn their habits. Derives the learning signature
  # — sender Contact, sender domain, category — server-side from a representative
  # email (never trusting the client), scoped to the user's own readable mail so a
  # forged id can't write a decision against someone else's sender.
  #
  # Best-effort and non-fatal: a triage action must never fail because we couldn't
  # log it, so any error is swallowed (logged) and the archive / keep / pin stands.
  class SkimDecisionRecorder
    def self.record(...) = new(...).call

    def initialize(user, email_ids, action:)
      @user = user
      @email_ids = Array(email_ids).map(&:to_s).reject(&:empty?)
      @action = action.to_s
    end

    def call
      return unless SkimDecision::ACTIONS.include?(@action)
      return if @email_ids.empty?

      email = representative
      return unless email

      decision = SkimDecision.create!(
        user: @user,
        workspace_id: @user.workspace_id,
        contact_id: email.contact_id,
        sender_domain: Emails::SenderDomain.for(email.from_address),
        category: Emails::Categorizer.new(email).call.category.to_s,
        email_message_id: email.id,
        action: @action
      )

      Events.publish(
        "email.skim_decision",
        subject: email,
        actor: @user,
        workspace: email.email_account.workspace,
        payload: { "decision" => @action, "subject" => email.subject.to_s }
      )

      decision
    rescue => e
      Rails.logger.warn("[SkimDecisionRecorder] #{e.class}: #{e.message}")
      nil
    end

    private

    # The cluster's most-recent email, scoped to mail the user may read. A Skim
    # cluster is a single sender, so one representative carries the right signature —
    # and it matches the representative SkimBuilder keys the suggestion on (the
    # newest email in the cluster), so record-time and lookup-time agree.
    def representative
      EmailMessage
        .where(email_account: @user.readable_email_accounts)
        .where(id: @email_ids)
        .order(received_at: :desc)
        .first
    end
  end
end

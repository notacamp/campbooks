# frozen_string_literal: true

module Emails
  # Single source of truth for sending an outbound email. Resolves the sending
  # account (enforcing the user's send permission), dispatches to the right
  # provider client (Gmail/Zoho expose a one-shot send_message; Microsoft Graph
  # does not, so fall back to save_draft → send_draft), threads replies via
  # In-Reply-To, records the local "sent" EmailMessage, and emits email.sent.
  #
  # Used by both the web composer (EmailComposeController) and the public API
  # (Api::V1::EmailMessagesController) so provider logic lives in exactly one place.
  #
  #   result = Emails::Sender.call(user:, to_address:, subject:, body:, ...)
  #   result.ok? # => true/false
  class Sender
    # Immutable result. On success carries the persisted EmailMessage (may be nil
    # if local recording failed even though the provider accepted the message)
    # and the provider's message id. On failure carries a stable error code.
    Result = Data.define(:ok, :email_message, :provider_message_id, :error_code, :error_message) do
      def ok? = ok

      def self.success(email_message:, provider_message_id:)
        new(ok: true, email_message: email_message, provider_message_id: provider_message_id,
            error_code: nil, error_message: nil)
      end

      def self.failure(code, message)
        new(ok: false, email_message: nil, provider_message_id: nil,
            error_code: code, error_message: message)
      end
    end

    def self.call(**kwargs)
      new(**kwargs).call
    end

    # source_message: the EmailMessage being replied to / forwarded (optional). It
    # provides reply threading, the existing thread, and the fallback account.
    def initialize(user:, to_address:, subject: nil, body: nil, email_account_id: nil,
                   cc_address: nil, bcc_address: nil, source_message: nil, attachments: [])
      @user = user
      @email_account_id = email_account_id
      @to_address = to_address.to_s.strip
      @subject = subject
      @body = body
      @cc_address = cc_address.presence
      @bcc_address = bcc_address.presence
      @source_message = source_message
      @attachments = attachments || []
    end

    def call
      account = resolve_account
      return Result.failure("no_sendable_account", "No email account is available to send from.") unless account
      return Result.failure("recipient_required", "A recipient (to_address) is required.") if @to_address.blank?

      provider_message_id = deliver(account.mail_client)
      unless provider_message_id
        return Result.failure("send_failed", "The email provider rejected or failed to send the message.")
      end

      sent = record_sent_message(account, provider_message_id)
      publish_event(account)
      Result.success(email_message: sent, provider_message_id: provider_message_id)
    end

    private

    attr_reader :user, :to_address, :subject, :body, :cc_address, :bcc_address,
                :source_message, :attachments

    # Explicit account wins; otherwise reply from the source message's account —
    # but only if the user may SEND from it (read access to a shared inbox must
    # not grant send).
    def resolve_account
      if @email_account_id.present?
        user.sendable_email_accounts.find_by(id: @email_account_id)
      elsif source_message
        account = source_message.email_account
        account if account&.sendable_by?(user)
      end
    end

    # Returns the provider message id, or nil on any provider failure.
    def deliver(mail_client)
      if mail_client.respond_to?(:send_message)
        result = mail_client.send_message(
          subject: subject,
          body: body.to_s,
          to_address: to_address,
          cc_address: cc_address,
          attachments: attachments
        )
        # Don't log the result — it echoes recipients/subject. A bare marker suffices.
        Rails.logger.info("[Emails::Sender] message sent via provider")
        extract_id(result)
      else
        draft = mail_client.save_draft(
          subject: subject,
          body: body.to_s,
          to_address: to_address,
          cc_address: cc_address,
          in_reply_to_message_id: source_message&.provider_message_id,
          attachments: attachments
        )
        draft_id = extract_id(draft)
        return nil unless draft_id

        mail_client.send_draft(draft_id)
        draft_id
      end
    rescue => e
      Rails.logger.error("[Emails::Sender] deliver failed: #{e.message}")
      nil
    end

    def extract_id(result)
      data = result.is_a?(Hash) ? (result["data"] || result) : result
      data = data.first if data.is_a?(Array)
      data.is_a?(Hash) ? (data["messageId"] || data["id"]) : nil
    end

    # Best-effort local record: the provider already accepted the message, so a
    # recording failure is logged but does not fail the send (mirrors the prior
    # controller behavior). Returns the EmailMessage or nil.
    def record_sent_message(account, provider_message_id)
      thread = source_message&.email_thread ||
        Emails::Threading.find_or_create_outbound(account, subject.presence || "(no subject)")

      sent = thread.email_messages.create!(
        email_account: account,
        provider_message_id: provider_message_id,
        provider_folder_id: "sent",
        from_address: account.email_address,
        to_address: to_address,
        cc_address: cc_address,
        bcc_address: bcc_address,
        subject: subject,
        body: body.to_s,
        received_at: Time.current,
        read: true,
        status: :processed
      )

      # Mark the thread as awaiting their reply at once (don't wait for the provider
      # to sync the sent copy back) and let the AI judge a follow-up. Job gates on
      # AI config.
      thread.update_column(:last_outbound_at, sent.received_at)
      Emails::FollowUpAnalysisJob.perform_later(thread.id, sent.id)
      sent
    rescue => e
      Rails.logger.error("[Emails::Sender] failed to record sent message: #{e.message}")
      nil
    end

    def publish_event(account)
      Events.publish(
        "email.sent",
        subject: nil,
        workspace: account.workspace,
        payload: { "subject" => subject, "to" => to_address }
      )
    end
  end
end

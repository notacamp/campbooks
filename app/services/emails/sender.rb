# frozen_string_literal: true

module Emails
  class Sender
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

    def self.call(**kwargs) = new(**kwargs).call

    def initialize(user:, to_address:, subject: nil, body: nil, email_account_id: nil,
                   cc_address: nil, bcc_address: nil, source_message: nil, attachments: [],
                   attachment_signed_ids: [])
      @user = user
      @email_account_id = email_account_id
      @to_address = to_address.to_s.strip
      @subject = subject
      @body = body
      @cc_address = cc_address.presence
      @bcc_address = bcc_address.presence
      @source_message = source_message
      @attachments = attachments || []
      @attachment_signed_ids = attachment_signed_ids || []
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
      if sent && @attachment_signed_ids.present?
        sent.update_column(:has_attachment, true)
        Emails::SentAttachmentProcessJob.perform_later(sent.id, user.id, @attachment_signed_ids)
      end
      Result.success(email_message: sent, provider_message_id: provider_message_id)
    end

    private

    attr_reader :user, :to_address, :subject, :body, :cc_address, :bcc_address,
                :source_message, :attachments, :attachment_signed_ids

    def resolve_account
      if @email_account_id.present?
        user.sendable_email_accounts.find_by(id: @email_account_id)
      elsif source_message
        account = source_message.email_account
        account if account&.sendable_by?(user)
      end
    end

    def deliver(mail_client)
      if mail_client.respond_to?(:send_message)
        result = mail_client.send_message(
          subject: subject, body: body.to_s, to_address: to_address,
          cc_address: cc_address, attachments: attachments
        )
        Rails.logger.info("[Emails::Sender] message sent via provider")
        extract_id(result)
      else
        draft = mail_client.save_draft(
          subject: subject, body: body.to_s, to_address: to_address,
          cc_address: cc_address, in_reply_to_message_id: source_message&.provider_message_id,
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

    def record_sent_message(account, provider_message_id)
      thread = source_message&.email_thread ||
        Emails::Threading.find_or_create_outbound(account, subject.presence || "(no subject)")
      sent = thread.email_messages.create!(
        email_account: account, provider_message_id: provider_message_id,
        provider_folder_id: "sent", from_address: account.email_address,
        to_address: to_address, cc_address: cc_address, bcc_address: bcc_address,
        subject: subject, body: body.to_s, received_at: Time.current,
        read: true, status: :processed
      )
      thread.update_column(:last_outbound_at, sent.received_at)
      Emails::FollowUpAnalysisJob.perform_later(thread.id, sent.id)
      sent
    rescue => e
      Rails.logger.error("[Emails::Sender] failed to record sent message: #{e.message}")
      nil
    end

    def publish_event(account)
      Events.publish(
        "email.sent", subject: nil, workspace: account.workspace,
        payload: { "subject" => subject, "to" => to_address }
      )
    end
  end
end

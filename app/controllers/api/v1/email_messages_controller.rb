# frozen_string_literal: true

module Api
  module V1
    # Public API for email. All reads go through EmailMessage.accessible_to so a
    # token only ever sees mail from accounts its acting user can read; sends
    # additionally require the acting user's send permission on the account
    # (enforced inside Emails::Sender).
    class EmailMessagesController < BaseController
      before_action -> { doorkeeper_authorize! :"emails:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"emails:write" }, only: [ :mark_read, :mark_unread ]
      before_action -> { doorkeeper_authorize! :"emails:send" },  only: [ :create, :reply ]
      before_action :set_email, only: [ :show, :mark_read, :mark_unread, :reply ]

      def index
        scope = EmailMessage.accessible_to(Current.user)
                            .includes(:tags)
                            .order(received_at: :desc)
        scope = apply_filters(scope)
        @pagy, emails = pagy(scope, limit: per_page)
        render_page(emails.map { |email| EmailSerializer.new(email).as_json }, @pagy)
      end

      def show
        render_data(EmailSerializer.new(@email, detail: true).as_json)
      end

      # Marks read locally and syncs the flag to the provider mailbox.
      def mark_read
        @email.update!(read: true)
        MarkReadJob.perform_later(@email.email_account_id, [ @email.provider_message_id ])
        render_data(EmailSerializer.new(@email, detail: true).as_json)
      end

      # Local-only: there is no cross-provider "mark unread" path.
      def mark_unread
        @email.update!(read: false)
        render_data(EmailSerializer.new(@email, detail: true).as_json)
      end

      # Send a brand-new message. email_account_id + to_address are required.
      def create
        result = Emails::Sender.call(
          user: Current.user,
          email_account_id: params.require(:email_account_id),
          to_address: params.require(:to_address),
          subject: params[:subject],
          body: params[:body],
          cc_address: params[:cc_address],
          bcc_address: params[:bcc_address]
        )
        render_send_result(result)
      end

      # Reply (or reply-all) to a message the token can see. Threads automatically
      # via the source message; sends from its account unless email_account_id is given.
      def reply
        result = Emails::Sender.call(
          user: Current.user,
          source_message: @email,
          email_account_id: params[:email_account_id],
          to_address: params[:to_address].presence || @email.from_address,
          cc_address: params[:cc_address],
          bcc_address: params[:bcc_address],
          subject: reply_subject,
          body: params.require(:body)
        )
        render_send_result(result)
      end

      private

      def set_email
        @email = EmailMessage.accessible_to(Current.user).find(params[:id])
      end

      def reply_subject
        subject = @email.subject.to_s
        subject.match?(/\Are:/i) ? subject : "Re: #{subject}"
      end

      def render_send_result(result)
        if result.ok?
          render_data({ id: result.email_message&.id, provider_message_id: result.provider_message_id },
                      status: :created)
        else
          status = result.error_code == "no_sendable_account" ? :forbidden : :unprocessable_entity
          render_api_error(result.error_code, result.error_message, status: status)
        end
      end

      def apply_filters(scope)
        if params[:account_ids].present?
          scope = scope.where(email_account_id: Array(params[:account_ids]))
        end

        unless params[:unread].nil?
          unread = ActiveModel::Type::Boolean.new.cast(params[:unread])
          scope = scope.where(read: !unread) unless unread.nil?
        end

        if params[:has_attachment].present?
          scope = scope.where(has_attachment: ActiveModel::Type::Boolean.new.cast(params[:has_attachment]))
        end

        scope = scope.where(category: params[:category]) if params[:category].present?

        if params[:priority].present? && EmailMessage.ai_priorities.key?(params[:priority])
          scope = scope.where(ai_priority: params[:priority])
        end

        if params[:q].present?
          like = "%#{params[:q]}%"
          scope = scope.where("email_messages.subject ILIKE :q OR email_messages.from_address ILIKE :q", q: like)
        end

        if (after = parse_time(params[:received_after]))
          scope = scope.where("email_messages.received_at >= ?", after)
        end

        if (before = parse_time(params[:received_before]))
          scope = scope.where("email_messages.received_at <= ?", before)
        end

        scope
      end

      def parse_time(value)
        return nil if value.blank?

        Time.zone.parse(value.to_s)
      rescue ArgumentError
        nil
      end
    end
  end
end

# frozen_string_literal: true

module Api
  module V1
    # Public API for compose drafts (DraftEmail) — the autosaved, unsent state
    # behind the web composer. Drafts are strictly private to their author, so
    # everything is scoped through Current.user.draft_emails; a token acting as a
    # given user sees only that user's drafts. Sending is a separate step: POST a
    # finished draft's fields to /emails or /emails/:id/reply.
    #
    # Foreign in_reply_to / email_account ids are dropped (nulled), never leaked —
    # the same lenient handling the web autosave endpoint uses.
    class DraftsController < BaseController
      before_action -> { doorkeeper_authorize! :"drafts:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"drafts:write" }, only: [ :create, :update, :destroy ]
      before_action :set_draft, only: [ :show, :update, :destroy ]

      def index
        scope = Current.user.draft_emails.latest_first
        @pagy, drafts = pagy(scope, limit: per_page)
        render_page(drafts.map { |d| DraftSerializer.new(d).as_json }, @pagy)
      end

      def show
        render_data(DraftSerializer.new(@draft).as_json)
      end

      def create
        draft = Current.user.draft_emails.new(draft_attributes)
        draft.workspace = Current.workspace
        draft.in_reply_to = accessible_message(params[:in_reply_to_id])
        draft.email_account = sendable_account(params[:email_account_id])

        if draft.save
          DraftEmail.prune_for(Current.user)
          render_data(DraftSerializer.new(draft).as_json, status: :created)
        else
          render_api_error("validation_failed", draft.errors.full_messages.to_sentence,
                           status: :unprocessable_entity)
        end
      end

      def update
        attrs = draft_attributes
        attrs[:email_account] = sendable_account(params[:email_account_id]) if params.key?(:email_account_id)
        attrs[:in_reply_to] = accessible_message(params[:in_reply_to_id]) if params.key?(:in_reply_to_id)
        # `dismissed` is an explicit boolean over the API (the web revives a draft
        # on any edit; an API client controls the pill state directly).
        unless params[:dismissed].nil?
          attrs[:dismissed_at] = ActiveModel::Type::Boolean.new.cast(params[:dismissed]) ? Time.current : nil
        end

        if @draft.update(attrs)
          render_data(DraftSerializer.new(@draft).as_json)
        else
          render_api_error("validation_failed", @draft.errors.full_messages.to_sentence,
                           status: :unprocessable_entity)
        end
      end

      def destroy
        @draft.destroy
        head :no_content
      end

      private

      def set_draft
        @draft = Current.user.draft_emails.find(params[:id])
      end

      # Only content fields are mass-assignable; the associations (account, reply,
      # dismissal) are resolved explicitly above with their own permission checks.
      def draft_attributes
        attrs = params.permit(
          :mode, :to, :cc, :bcc, :subject, :body, :quoted_body, :signature_id,
          attachments: [ :signed_id, :filename, :byte_size ]
        ).to_h.symbolize_keys

        # Map the API's friendly names onto the model columns.
        remap = { to: :to_address, cc: :cc_address, bcc: :bcc_address, attachments: :attachments_json }
        remap.each { |from, to| attrs[to] = attrs.delete(from) if attrs.key?(from) }

        # The signature must be the acting user's own; a stray id is dropped.
        if attrs[:signature_id].present?
          attrs[:signature_id] = Current.user.signatures.find_by(id: attrs[:signature_id])&.id
        end
        attrs
      end

      def accessible_message(id)
        return nil if id.blank?

        EmailMessage.accessible_to(Current.user).find_by(id: id)
      end

      def sendable_account(id)
        return nil if id.blank?

        Current.user.sendable_email_accounts.find_by(id: id)
      end
    end
  end
end

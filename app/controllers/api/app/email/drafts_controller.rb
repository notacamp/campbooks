# frozen_string_literal: true

module Api
  module App
    module Email
      # Draft autosave for the React compose surfaces. The SPA POSTs on first
      # keystroke (create), PATCHes on a debounced interval (update), DELETEs on
      # explicit discard, and uses dismiss/undismiss to control the dock pill.
      # Drafts are strictly private — scoped through Current.user.draft_emails.
      # Delegates serialization to Api::App::Email::DraftSerializer (which wraps
      # Api::V1::DraftSerializer and adds no new fields currently).
      class DraftsController < BaseController
        before_action :set_draft, only: %i[show update destroy dismiss undismiss]

        def index
          scope = current_user.draft_emails.latest_first
          @pagy, drafts = pagy(scope, limit: per_page)
          render_page(drafts.map { |d| DraftSerializer.new(d).as_json }, @pagy)
        end

        def show
          render_data(DraftSerializer.new(@draft).as_json)
        end

        def create
          draft = current_user.draft_emails.new(draft_attributes)
          draft.workspace = current_workspace
          draft.in_reply_to = accessible_message(params[:in_reply_to_id])
          draft.email_account = sendable_account(params[:email_account_id])

          if draft.save
            ::DraftEmail.prune_for(current_user)
            render_data(DraftSerializer.new(draft).as_json, status: :created)
          else
            render_error("invalid", draft.errors.full_messages.to_sentence,
                         status: :unprocessable_entity)
          end
        end

        def update
          attrs = draft_attributes
          attrs[:email_account] = sendable_account(params[:email_account_id]) if params.key?(:email_account_id)
          attrs[:in_reply_to]   = accessible_message(params[:in_reply_to_id])  if params.key?(:in_reply_to_id)
          # Any edit revives a dismissed draft.
          attrs[:dismissed_at] = nil

          if @draft.update(attrs)
            render_data(DraftSerializer.new(@draft).as_json)
          else
            render_error("invalid", @draft.errors.full_messages.to_sentence,
                         status: :unprocessable_entity)
          end
        end

        def destroy
          @draft.destroy
          head :no_content
        end

        def dismiss
          @draft.update!(dismissed_at: ::Time.current)
          render_data(DraftSerializer.new(@draft).as_json)
        end

        def undismiss
          @draft.update!(dismissed_at: nil)
          render_data(DraftSerializer.new(@draft).as_json)
        end

        private

        def set_draft
          @draft = current_user.draft_emails.find(params[:id])
        end

        def draft_attributes
          attrs = params.permit(
            :mode, :to, :cc, :bcc, :subject, :body, :quoted_body, :signature_id,
            attachments: %i[signed_id filename byte_size]
          ).to_h.symbolize_keys

          # Map friendly param names to model column names.
          remap = { to: :to_address, cc: :cc_address, bcc: :bcc_address, attachments: :attachments_json }
          remap.each { |from, to| attrs[to] = attrs.delete(from) if attrs.key?(from) }

          # Guard signature ownership.
          if attrs[:signature_id].present?
            attrs[:signature_id] = current_user.signatures.find_by(id: attrs[:signature_id])&.id
          end

          attrs
        end

        def accessible_message(id)
          return nil if id.blank?

          ::EmailMessage.accessible_to(current_user).find_by(id: id)
        end

        def sendable_account(id)
          return nil if id.blank?

          current_user.sendable_email_accounts.find_by(id: id)
        end
      end
    end
  end
end

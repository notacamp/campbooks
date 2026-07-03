# frozen_string_literal: true

# JSON autosave endpoint behind both compose surfaces (Dock + Desk). The
# compose-autosave Stimulus controller POSTs the first time the user types,
# PATCHes on a debounce afterwards, and DELETEs on explicit discard. Drafts are
# strictly private to their author — everything is scoped through
# Current.user.draft_emails, and a foreign in_reply_to id is dropped rather
# than leaked (404-not-403 rule doesn't apply: we just null the association).
class DraftEmailsController < ApplicationController
  before_action :require_authentication
  before_action :load_draft, only: [ :update, :destroy ]

  # Re-opens a parked draft in the Dock (the pill's click target). A stale pill
  # pointing at a consumed draft just removes itself.
  def show
    draft = Current.user.draft_emails.find_by(id: params[:id])
    return stale_pill_response unless draft

    message = draft.in_reply_to
    respond_to do |format|
      format.turbo_stream do
        render turbo_stream: turbo_stream.update("compose_dock", partial: "email_compose/dock", locals: {
          mode: draft.mode.to_sym,
          message: message,
          draft: draft,
          to: draft.to_address.to_s,
          cc: draft.cc_address.to_s,
          bcc: draft.bcc_address.to_s,
          subject: draft.subject.to_s,
          body: draft.body.to_s,
          quoted_body: draft.quoted_body.to_s,
          signatures: Current.user.signatures.ordered.includes(:email_accounts),
          signature_id: draft.signature_id,
          accounts: draft.new_message? ? Current.user.sendable_email_accounts.to_a : [],
          attachment_entries: draft.attachment_entries
        })
      end
      format.html { redirect_to message ? email_message_path(message) : email_messages_path }
    end
  end

  def create
    draft = Current.user.draft_emails.new(draft_params)
    draft.workspace = Current.workspace
    draft.in_reply_to = accessible_message(params.dig(:draft_email, :in_reply_to_id))
    draft.email_account = sendable_account(params.dig(:draft_email, :email_account_id))

    if draft.save
      DraftEmail.prune_for(Current.user)
      render json: { id: draft.id, saved_at: draft.updated_at.iso8601 }, status: :created
    else
      render json: { error: draft.errors.full_messages.to_sentence }, status: :unprocessable_entity
    end
  end

  def update
    attrs = draft_params
    attrs[:email_account] = sendable_account(params.dig(:draft_email, :email_account_id)) if params.dig(:draft_email, :email_account_id)

    if @draft.update(attrs)
      render json: { id: @draft.id, saved_at: @draft.updated_at.iso8601 }
    else
      render json: { error: @draft.errors.full_messages.to_sentence }, status: :unprocessable_entity
    end
  end

  def destroy
    @draft.destroy
    head :no_content
  end

  private

  def stale_pill_response
    respond_to do |format|
      format.turbo_stream { render turbo_stream: turbo_stream.remove("compose_draft_pill") }
      format.html { redirect_to email_messages_path }
    end
  end

  def load_draft
    @draft = Current.user.draft_emails.find(params[:id])
  end

  def draft_params
    permitted = params.require(:draft_email).permit(
      :mode, :to_address, :cc_address, :bcc_address, :subject, :body, :quoted_body, :signature_id,
      attachments_json: [ :signed_id, :filename, :byte_size ]
    )
    # The signature must be the user's own; a stray id is dropped, not an error.
    if permitted[:signature_id].present?
      permitted[:signature_id] = Current.user.signatures.find_by(id: permitted[:signature_id])&.id
    end
    permitted
  end

  # Only messages the user can read may be linked as the replied-to thread.
  def accessible_message(id)
    return nil if id.blank?

    EmailMessage.accessible_to(Current.user).find_by(id: id)
  end

  # Only accounts the user can send from may be stored on a draft.
  def sendable_account(id)
    return nil if id.blank?

    Current.user.sendable_email_accounts.find_by(id: id)
  end
end

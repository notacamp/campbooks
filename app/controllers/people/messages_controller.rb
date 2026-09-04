# frozen_string_literal: true

module People
  # Lazily loads the body of one folded message in a person's conversation. The
  # ThreadMessage row renders its always-visible summary line up front; the body
  # (and the attachments row) arrives through this frame when the row is expanded.
  #
  # Scoped like the conversation itself: the message must be readable by the
  # current user and belong to a thread of this person's. Anything else is
  # invisible (404), not forbidden.
  class MessagesController < ApplicationController
    include PeopleLayout

    def show
      @person = Current.workspace.people.find(params[:id])
      @message = EmailMessage.accessible_to(current_user)
                             .includes(:contact, :email_account, files_attachments: :blob)
                             .find(params[:message_id])
      return head :not_found unless on_conversation?(@message)

      render :show, layout: false
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    private

    def on_conversation?(message)
      contact_ids = @person.contacts.ids
      return false if contact_ids.empty?
      return true if contact_ids.include?(message.contact_id)
      return false if message.email_thread_id.blank?

      EmailMessage.where(email_thread_id: message.email_thread_id, contact_id: contact_ids).exists?
    end
  end
end

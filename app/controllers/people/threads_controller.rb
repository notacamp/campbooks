# frozen_string_literal: true

module People
  # Lazily loads one thread of a person's conversation. The person page renders
  # every thread but the newest as a heading over a lazy turbo-frame; this action
  # fills the frame with the thread's messages when it scrolls into view, and
  # marks the thread read the way opening it in the inbox would — its newest
  # message is now on screen.
  #
  # Scoped like the conversation itself: the thread must carry a message from one
  # of this person's contacts, and only messages the current user may read are
  # rendered. Anything else is invisible (404), not forbidden.
  class ThreadsController < ApplicationController
    include PeopleLayout

    def show
      @person = Current.workspace.people.find(params[:id])
      @thread = EmailThread.find(params[:thread_id])
      messages = conversation_messages(@thread)
      return head :not_found if messages.empty?

      if Emails::MarkThreadRead.call(@thread)
        People::Standings.refresh_counterpart!(current_user, @person)
      end
      @conversation_thread = People::ConversationThread.new(thread: @thread, messages: messages)
      @can_send = @thread.email_account.sendable_by?(current_user)
      render :show, layout: false
    rescue ActiveRecord::RecordNotFound
      head :not_found
    end

    private

    # The thread's accessible messages, oldest first — or none when the thread is
    # not part of this person's conversation.
    def conversation_messages(thread)
      contact_ids = @person.contacts.ids
      return [] if contact_ids.empty?

      messages = EmailMessage.where(email_thread_id: thread.id)
                             .accessible_to(current_user)
                             .includes(:contact, :email_account, files_attachments: :blob)
                             .order(:received_at, :id).to_a
      messages.any? { |m| contact_ids.include?(m.contact_id) } ? messages : []
    end
  end
end

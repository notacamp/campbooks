# frozen_string_literal: true

module Api
  module App
    module People
      # GET /api/app/people/:person_id/threads/:id
      # Lazy-load one conversation thread's messages. The SPA calls this on expand
      # (mirroring People::ThreadsController in the web app).
      class ThreadsController < Api::App::BaseController
        def show
          person = current_workspace.people.find(params[:person_id])
          contact_ids = person.contacts.ids

          # Verify the thread belongs to this person.
          thread = EmailThread.where(id: params[:id]).first
          return render_not_found unless thread

          # Gate: the thread must have messages readable by the current user AND
          # associated with this person's contacts.
          message_ids = EmailMessage.where(email_thread_id: thread.id)
                                    .where(contact_id: contact_ids)
                                    .accessible_to(current_user)
                                    .pluck(:id)
          return render_not_found if message_ids.empty?

          messages = EmailMessage.where(id: message_ids)
                                 .includes(:contact, :email_account, files_attachments: :blob)
                                 .order(:received_at, :id).to_a

          heads = EmailMessage.where(email_thread_id: thread.id)
                              .accessible_to(current_user)
                              .pluck(:email_thread_id, :id, :received_at)
          _tid, newest_id, latest_at = heads.max_by { |(_t, _id, at)| at || ::Time.at(0) }

          ct = ::People::ConversationThread.new(
            thread:   thread,
            messages: messages,
            count:    heads.size,
            latest_at: latest_at,
            newest_id: newest_id
          )

          render_data(Api::App::ConversationThreadSerializer.new(ct).as_json)
        end
      end
    end
  end
end

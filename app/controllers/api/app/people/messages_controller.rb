# frozen_string_literal: true

module Api
  module App
    module People
      # GET /api/app/people/:person_id/messages/:id
      # Lazy-load one message body (the folded-message expand in a thread).
      class MessagesController < Api::App::BaseController
        def show
          person = current_workspace.people.find(params[:person_id])
          contact_ids = person.contacts.ids

          message = EmailMessage.where(contact_id: contact_ids)
                                .accessible_to(current_user)
                                .includes(:contact, :email_account, files_attachments: :blob)
                                .find(params[:id])

          render_data(Api::App::MessageSerializer.new(message).as_json)
        end
      end
    end
  end
end

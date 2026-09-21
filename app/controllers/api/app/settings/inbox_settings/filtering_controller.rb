# frozen_string_literal: true

module Api
  module App
    module Settings
      module InboxSettings
        # Inbox filter strategy + sender allow/block/star management.
        class FilteringController < Api::App::BaseController
          # GET /api/app/inbox_settings/filtering
          def show
            render_data filtering_data
          end

          # PATCH /api/app/inbox_settings/filtering
          def update
            strategy = params[:inbox_filter_strategy].to_s
            if Workspace::INBOX_FILTER_STRATEGIES.include?(strategy)
              ws = current_workspace
              ws.update(settings: ws.settings.merge("inbox_filter_strategy" => strategy))
            end
            render_data filtering_data
          end

          # POST /api/app/inbox_settings/filtering/sender
          def set_sender
            contact = current_workspace.contacts.find_by(id: params[:contact_id])
            apply_state(contact, params[:state].to_s) if contact
            render_data filtering_data
          end

          private

          def apply_state(contact, state)
            case state
            when "unblock" then Contacts::Unblock.call(contact, user: current_user)
            when "neutral" then contact.unblock!
            when "unstar"  then contact.unstar!
            when "block"   then Contacts::Block.call(contact, user: current_user)
            when "allow"   then contact.allow!
            end
          end

          def filtering_data
            contacts = current_workspace.contacts
            {
              strategy: current_workspace.inbox_filter_strategy,
              starred: contacts.starred.order(:name, :email).limit(50).map { |c| contact_data(c) },
              blocked: contacts.blocked.order(:name, :email).limit(50).map { |c| contact_data(c) },
              allowed: contacts.allowed.order(:name, :email).limit(50).map { |c| contact_data(c) }
            }
          end

          def contact_data(contact)
            { id: contact.id, name: contact.name, email: contact.email }
          end
        end
      end
    end
  end
end

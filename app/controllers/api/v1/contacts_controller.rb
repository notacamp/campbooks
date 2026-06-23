# frozen_string_literal: true

module Api
  module V1
    # Public API for contacts. Contacts are auto-created from email sync, so there
    # is no create endpoint — only read, edit (name/relationship, via the linked
    # Person), and state changes (star/block/allow). Workspace-scoped.
    class ContactsController < BaseController
      before_action -> { doorkeeper_authorize! :"contacts:read" },  only: [ :index, :show ]
      before_action -> { doorkeeper_authorize! :"contacts:write" }, only: [ :update, :state ]
      before_action :set_contact, only: [ :show, :update, :state ]

      def index
        scope = Current.workspace.contacts
        if params[:list_status].present? && Contact.list_statuses.key?(params[:list_status])
          scope = scope.where(list_status: params[:list_status])
        end
        scope = scope.starred if ActiveModel::Type::Boolean.new.cast(params[:starred])
        if params[:q].present?
          like = "%#{params[:q]}%"
          scope = scope.where("contacts.name ILIKE :q OR contacts.email ILIKE :q", q: like)
        end

        @pagy, contacts = pagy(scope.order(:name), limit: per_page)
        render_page(contacts.map { |contact| ContactSerializer.new(contact).as_json }, @pagy)
      end

      def show
        render_data(ContactSerializer.new(@contact).as_json)
      end

      # Edits flow through the linked Person (the editable identity); the contact's
      # denormalized name/relationship columns are kept in sync.
      def update
        person = @contact.person || Person.create!(workspace: Current.workspace)
        @contact.update!(person: person) unless @contact.person_id == person.id
        person.update!(person_attributes)
        @contact.update_columns(name: person.name, relationship_type: person.relationship_type)
        render_data(ContactSerializer.new(@contact.reload).as_json)
      end

      # POST /api/v1/contacts/:id/state  body: { state: star|unstar|allow|block|unblock }
      def state
        case params[:state]
        when "star"    then @contact.star!
        when "unstar"  then @contact.unstar!
        when "allow"   then @contact.allow!
        when "block"   then Contacts::Block.call(@contact, user: current_user)
        when "unblock" then Contacts::Unblock.call(@contact, user: current_user)
        else
          return render_api_error("invalid_state",
                                  "state must be one of: star, unstar, allow, block, unblock.",
                                  status: :unprocessable_entity)
        end

        render_data(ContactSerializer.new(@contact.reload).as_json)
      end

      private

      def set_contact
        @contact = Current.workspace.contacts.find(params[:id])
      end

      # Only the fields the client actually sent (so a partial update doesn't blank
      # the other).
      def person_attributes
        { name: params[:name], relationship_type: params[:relationship_type] }.compact
      end
    end
  end
end

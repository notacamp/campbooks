# frozen_string_literal: true

module Api
  module App
    module Settings
      # Pending or resolved workspace invitation.
      class InvitationSerializer
        def initialize(invitation)
          @invitation = invitation
        end

        def as_json(*)
          {
            id: @invitation.id,
            email: @invitation.email,
            status: @invitation.status,
            admin_approved: @invitation.admin_approved,
            expires_at: @invitation.expires_at,
            created_at: @invitation.created_at,
            invited_by: invited_by_data
          }
        end

        private

        def invited_by_data
          return nil unless @invitation.invited_by

          {
            id: @invitation.invited_by.id,
            name: @invitation.invited_by.name,
            email: @invitation.invited_by.email_address
          }
        end
      end
    end
  end
end

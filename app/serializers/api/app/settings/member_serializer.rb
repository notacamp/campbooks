# frozen_string_literal: true

module Api
  module App
    module Settings
      # Workspace member: user identity + workspace role.
      class MemberSerializer
        def initialize(user)
          @user = user
        end

        def as_json(*)
          {
            id: @user.id,
            name: @user.name,
            email: @user.email_address,
            role: @user.role,
            created_at: @user.created_at
          }
        end
      end
    end
  end
end

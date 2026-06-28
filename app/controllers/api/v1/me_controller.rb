# frozen_string_literal: true

module Api
  module V1
    # GET /api/v1/me — the identity behind the token: the acting user, their
    # workspace, and the token's granted scopes. Requires only a valid token (any
    # scope the CLI requests), so `campbooks login` / `whoami` can confirm who
    # they're signed in as without having to read another resource.
    class MeController < BaseController
      def show
        user = Current.acting_user
        render_data({
          user: { id: user.id, name: user.name, email: user.email_address },
          workspace: { id: Current.workspace.id, name: Current.workspace.name },
          scopes: Array(doorkeeper_token&.scopes).map(&:to_s).sort
        })
      end
    end
  end
end

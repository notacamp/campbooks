# frozen_string_literal: true

module Api
  module App
    # GET /api/app/me — the bootstrap payload the SPA loads on launch: the acting
    # identity, the workspace, and the deploy-level feature flags the client uses
    # for capability gates. Requires only a valid session bearer. See
    # api-migration/00-auth.md.
    class MeController < BaseController
      def show
        render_data(MeSerializer.new(Current.user, Current.workspace).as_json)
      end
    end
  end
end

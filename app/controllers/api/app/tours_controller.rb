# frozen_string_literal: true

module Api
  module App
    # Tour dismissal for the first-party app API.
    #
    # Records that the current user has seen a one-time guided overlay so it
    # won't appear again. Mirrors the web ToursController (fire-and-forget).
    # Unknown keys are stored silently (forward-compatible with future tours).
    #
    #   POST /api/app/tours/:key/dismiss
    class ToursController < BaseController
      # POST /api/app/tours/:key/dismiss
      def dismiss
        current_user.dismiss_tour!(params[:key])
        head :no_content
      end
    end
  end
end

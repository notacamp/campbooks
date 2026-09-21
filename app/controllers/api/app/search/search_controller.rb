# frozen_string_literal: true

module Api
  module App
    module Search
      # Global search (Cmd+K) for the SPA. Delegates to GlobalSearch.call.
      class SearchController < Api::App::BaseController
        # GET /api/app/search?q=&types[]=
        def index
          q     = params[:q].to_s.strip
          types = params[:types].present? ? Array(params[:types]) : nil

          results = GlobalSearch.call(q, user: current_user, types: types)
          render_data(Api::App::SearchResultSerializer.new(results).as_json)
        rescue => e
          Rails.logger.error("[api/search] #{e.class}: #{e.message}")
          render_data(Api::App::SearchResultSerializer.new([]).as_json)
        end
      end
    end
  end
end

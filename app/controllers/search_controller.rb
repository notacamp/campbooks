class SearchController < ApplicationController
  before_action :require_authentication

  # GET /search?q=...  → JSON for the Cmd+K command palette.
  def index
    render json: { results: GlobalSearch.call(params[:q], user: Current.user, types: params[:types]) }
  rescue => e
    Rails.logger.error("[Search] #{e.class}: #{e.message}")
    render json: { results: [] }
  end
end

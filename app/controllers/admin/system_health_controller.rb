# frozen_string_literal: true

class Admin::SystemHealthController < Admin::BaseController
  WINDOWS = { "24h" => 24.hours, "7d" => 7.days, "30d" => 30.days }.freeze

  def show
    @snapshot = SystemHealth::Snapshot.new(window: 24.hours)
    scope = ExternalServiceCall.recent
    scope = scope.for_service(params[:service]) if params[:service].present?
    scope = scope.status_error   if params[:status] == "error"
    scope = scope.status_success if params[:status] == "success"
    scope = scope.since(WINDOWS.fetch(params[:window], 24.hours).ago)
    @pagy, @calls = pagy(scope, items: 50)
    respond_to do |format|
      format.html
      format.turbo_stream
    end
  end

  def call
    @call = ExternalServiceCall.find(params[:id])
  end
end

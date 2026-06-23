class Settings::BaseController < ApplicationController
  before_action :require_authentication

  private

  def current_section
    controller_name
  end
  helper_method :current_section
end

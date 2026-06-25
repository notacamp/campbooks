# Public liveness probe served at /up. Mirrors Rails' built-in rails/health#show
# (no authentication, no database access, returns 200 once the app has booted)
# but also reports the running version, so operators and self-hosters can confirm
# what's deployed with a plain `curl -s https://…/up`.
#
# Inherits ActionController::Base directly (not ApplicationController) to stay
# clear of the app's authentication, locale, and other around-actions — exactly
# as Rails' own health controller does. The single source of truth for the
# version is the VERSION file at the repo root (see config/application.rb).
class HealthController < ActionController::Base
  def show
    render json: { status: "ok", version: Campbooks::VERSION }
  end
end

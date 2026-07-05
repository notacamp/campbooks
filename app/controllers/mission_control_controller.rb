# Base controller for the Mission Control Jobs dashboard mounted at /jobs.
#
# Mission Control ships optional HTTP basic auth, which we disable
# (config/application.rb) in favour of the app's own login. Without a custom
# base controller the engine inherits plain ActionController::Base, leaving the
# queue dashboard — and the job arguments it renders, which can contain personal
# data (email IDs, addresses) — reachable by anyone who can hit the server, plus
# able to retry or discard jobs. This gates it behind the same session auth as
# the app and restricts it to admins.
class MissionControlController < ActionController::Base
  include Authentication

  before_action :require_admin

  private

  def require_admin
    # Instance operators only — the dashboard spans every workspace's jobs.
    head :forbidden unless Current.user&.app_admin?
  end
end

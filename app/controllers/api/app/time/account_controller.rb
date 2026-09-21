# frozen_string_literal: true

module Api
  module App
    module Time
      # PATCH /api/app/account/time_zone — one-shot zone capture. The SPA fires this
      # once (when the user's time_zone is blank) and forgets. Mirrors the
      # Settings::AccountController#time_zone logic exactly.
      class AccountController < Api::App::BaseController
        def time_zone
          zone = params[:time_zone].to_s
          if current_user.time_zone.blank? && zone.present? && ::Time.find_zone(zone)
            current_user.update(time_zone: zone)
          end
          head :no_content
        end
      end
    end
  end
end

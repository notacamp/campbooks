# Serves the Hotwire Native path configuration — the JSON ruleset the native
# iOS/Android shells fetch at launch to decide navigation behavior (which URLs
# open as modals, whether pull-to-refresh is enabled, etc.).
#
# Public + unauthenticated: the native app loads this before the user signs in.
# Versioned per platform via the :platform segment (e.g. `ios_v1`, `android_v1`)
# so installed apps can roll forward independently. Today every version serves
# the shared `path_configuration.json`; drop a `config/hotwire/<platform>.json`
# to diverge a single platform/version without touching this controller.
class ConfigurationsController < ApplicationController
  allow_unauthenticated_access

  CONFIGURATIONS_DIR = Rails.root.join("config/hotwire").freeze
  DEFAULT_CONFIGURATION = CONFIGURATIONS_DIR.join("path_configuration.json").freeze

  def show
    render json: File.read(configuration_file)
  end

  private
    # Resolve the requested platform/version to a real file in config/hotwire by
    # matching its basename — the path is never built from user input, so an
    # unknown (or hostile) slug simply falls back to the shared default.
    def configuration_file
      by_slug = CONFIGURATIONS_DIR.glob("*.json").index_by { |path| path.basename(".json").to_s }
      by_slug[params[:platform].to_s] || DEFAULT_CONFIGURATION
    end
end

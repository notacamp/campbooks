# frozen_string_literal: true

module Campbooks
  module Entitlements
    # Wraps content that needs a feature: renders the block when the workspace's
    # plan grants it, otherwise a compact UpgradePrompt banner in its place. Lets a
    # view gate a whole section without an if/else around it.
    #
    #   render Campbooks::Entitlements::FeatureLock.new(feature: :workflows, entitlements: current_entitlements) do
    #     # …the gated UI…
    #   end
    class FeatureLock < Campbooks::Base
      def initialize(feature:, entitlements:, reason: :not_allowed, **attrs)
        @feature = feature.to_sym
        @entitlements = entitlements
        @reason = reason
        @attrs = attrs
      end

      def view_template(&block)
        if @entitlements.feature?(@feature)
          yield if block
        else
          render Campbooks::Entitlements::UpgradePrompt.new(
            feature: @feature, reason: @reason, variant: :banner, **@attrs
          )
        end
      end
    end
  end
end

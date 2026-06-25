# frozen_string_literal: true

module Campbooks
  # The in-context "Processed by <provider> · <region>" affordance for an AI output,
  # built from a persisted provenance hash ({ "provider", "model", "region" }).
  # Renders nothing when provenance is absent (an older row, or AI was off).
  class AiProvenanceNote < Campbooks::Base
    def initialize(provenance:)
      @provenance = provenance || {}
    end

    def view_template
      return if provider.blank?

      span(class: "inline-flex items-center gap-1 text-[10px] text-muted-foreground") do
        span { t(".processed_by", provider: provider_label) }
        render Campbooks::AiRegionBadge.new(provider: provider)
      end
    end

    private

    def provider
      @provenance["provider"].presence
    end

    def provider_label
      helpers.human_enum(AiAdapter, :provider, provider)
    end
  end
end

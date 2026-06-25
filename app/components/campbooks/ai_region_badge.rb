# frozen_string_literal: true

module Campbooks
  # The data-region pill for an AI provider — EU = green, everything else = amber —
  # derived from AiConfiguration::PROVIDER_REGIONS. Renders nothing for an unknown
  # provider. Used in the AI settings adapter cards, the Data & Privacy page, and
  # the in-context "Processed by …" provenance affordances.
  class AiRegionBadge < Campbooks::Base
    def initialize(provider:)
      @provider = provider.to_s
    end

    def view_template
      return if region.blank?

      span(class: class_names(
        "ml-0.5 inline-flex items-center rounded px-1 text-[10px] font-medium",
        (eu? ? "bg-green-50 text-green-700 dark:bg-green-500/10 dark:text-green-400" :
               "bg-amber-50 text-amber-700 dark:bg-amber-500/10 dark:text-amber-400")
      )) { region }
    end

    private

    def region
      @region ||= AiConfiguration::PROVIDER_REGIONS[@provider]
    end

    def eu?
      region == "EU"
    end
  end
end

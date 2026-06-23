module Entitlements
  # Maps an ai_model_access tier (from a plan's config) to the set of model ids a
  # workspace may select. nil = unrestricted.
  #
  # Wired as a seam: every tier is unrestricted today. Narrowing a tier here
  # (e.g. "basic" => %w[mistral-small-latest gpt-4o-mini]) is the only change
  # needed to start gating premium models — the AiConfiguration validation and the
  # model picker already read this.
  module AiModelTiers
    TIERS = {
      "basic"    => nil,
      "standard" => nil,
      "premium"  => nil
    }.freeze

    module_function

    # Array of allowed model ids, or nil when the tier imposes no restriction.
    def models_for(tier)
      return nil if tier.blank?

      TIERS.fetch(tier.to_s, nil)
    end

    def restricted?(tier)
      !models_for(tier).nil?
    end
  end
end

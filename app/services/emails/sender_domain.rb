# frozen_string_literal: true

module Emails
  # The sender domain used as a clustering / learning key: the bare domain of a From
  # address collapsed to its last two labels (notifications.github.com → github.com).
  # Deliberately simpler than Emails::Categorizer's compound-TLD handling — it only
  # has to be STABLE and shared between the two sites that must agree: SkimBuilder
  # clusters by it, and SkimDecisionRecorder / SkimActionMemory key the learned
  # suggestion on it. If they computed it differently the memory would never match.
  module SenderDomain
    module_function

    def for(from_address)
      domain = from_address.to_s[/@([^>\s]+)/, 1].to_s.downcase
      parts = domain.split(".")
      parts.length > 2 ? parts.last(2).join(".") : domain
    end
  end
end

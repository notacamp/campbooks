module Ai
  # The legacy single-provider Anthropic fallback that AI services use when a
  # workspace has no configured provider for a purpose (Ai::Configuration.for
  # returns nil). It calls Anthropic directly on the shared ANTHROPIC_API_KEY.
  #
  # On a SELF-HOSTED install that key is the operator's OWN and the request stays
  # on infrastructure they control, so the fallback is allowed.
  #
  # On the managed CLOUD it would send user content (email bodies, chat, contacts)
  # to Anthropic (US) — outside the workspace's chosen/managed provider and with no
  # disclosure — a silent data-residency leak that contradicts the promise that
  # users control which AI processes their data. So it is DISABLED there: AI
  # features fail closed (return nil, exactly as when no provider is configured)
  # rather than quietly processing content on a key the user never picked.
  #
  # Healthy cloud workspaces are provisioned with managed AI at signup, so
  # Ai::Configuration.for resolves a real adapter and this fallback never runs.
  module LegacyFallback
    module_function

    def allowed?
      Rails.application.config.self_hosted && ENV["ANTHROPIC_API_KEY"].present?
    end
  end
end

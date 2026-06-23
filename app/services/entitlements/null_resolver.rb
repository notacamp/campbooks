module Entitlements
  # Self-hosted short-circuit: every feature active, no caps. Mirrors Resolver's
  # public API so call sites stay agnostic. (Managed AI remains blocked on
  # self-hosted by the existing AiAdapter validation, independent of this.)
  class NullResolver
    def plan_name = "unlimited"
    def feature?(_key) = true
    def limit(_key) = nil
    def config(_key, _subkey = nil) = nil
    def usage(_key) = nil
    def remaining(_key) = nil
    def over_cap?(_key) = false
    def allow?(_key) = :ok
    def allow!(_key) = true
    def summary = {}
  end
end

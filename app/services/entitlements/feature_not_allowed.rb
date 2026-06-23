module Entitlements
  # Raised when a feature is not granted by the plan (allowed: false) or has been
  # toggled off (enabled: false).
  class FeatureNotAllowed < Error; end
end

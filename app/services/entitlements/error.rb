module Entitlements
  # Base for entitlement enforcement errors. Carries the offending feature key so
  # callers/rescuers can render a feature-specific upgrade message.
  class Error < StandardError
    attr_reader :key

    def initialize(key:, message: nil)
      @key = key.to_sym
      super(message || "entitlement check failed for #{key}")
    end
  end
end

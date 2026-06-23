module Entitlements
  # Raised when a usage limit would be (or has been) exceeded.
  class LimitExceeded < Error
    attr_reader :limit

    def initialize(key:, limit:)
      @limit = limit
      super(key: key, message: "#{key} limit of #{limit} exceeded")
    end
  end
end

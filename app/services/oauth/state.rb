module Oauth
  # Signs (and reads) the OAuth `state` parameter.
  #
  # Historically `state` was an unsigned JSON blob (e.g. `{"flow":"sign_in"}`) —
  # trivially forgeable. Here it is HMAC-signed so it can safely carry identity
  # (a `user_id`) for the Hotwire Native auth-session handoff, where the callback
  # runs with no session cookie and must trust the state to know who is linking
  # an account. Signing also retroactively hardens the existing web flows.
  #
  # Output is url-safe and short-lived so it survives the OAuth redirect
  # round-trip and a stolen state can't be replayed later.
  class State
    EXPIRES_IN = 30.minutes

    class << self
      # Called with keyword fields, e.g. encode(flow: "sign_in", native: true).
      def encode(expires_in: EXPIRES_IN, **payload)
        verifier.generate(payload.compact.stringify_keys, expires_in: expires_in)
      end

      # Always returns a Hash with string keys. `"verified" => true` only when the
      # signature checked out — never trust identity/native fields unless verified.
      # A legacy unsigned JSON state decodes as unverified so old in-flight flows
      # (and the still-plain `drive_link` state) keep working across a deploy.
      def decode(raw)
        return {} if raw.blank?

        data = verifier.verify(raw)
        data.is_a?(Hash) ? data.merge("verified" => true) : {}
      rescue ActiveSupport::MessageVerifier::InvalidSignature
        legacy = JSON.parse(raw) rescue nil
        legacy.is_a?(Hash) ? legacy.merge("verified" => false) : {}
      end

      private
        def verifier
          @verifier ||= ActiveSupport::MessageVerifier.new(
            Rails.application.key_generator.generate_key("oauth_state"),
            url_safe: true,
            serializer: JSON
          )
        end
    end
  end
end

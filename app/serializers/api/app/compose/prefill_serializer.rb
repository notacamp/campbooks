# frozen_string_literal: true

module Api
  module App
    module Compose
      # Serializes an Emails::IntentPrefill::Result for the compose-prefill
      # endpoint. The SPA renders To + Subject as "· inferred" chips until the
      # user edits them; the *_inferred flags drive that UI state.
      class PrefillSerializer
        def initialize(result)
          @result = result
        end

        def as_json
          {
            to: @result.to,
            subject: @result.subject,
            to_inferred: @result.to_inferred?,
            subject_inferred: @result.subject_inferred?
          }
        end
      end
    end
  end
end

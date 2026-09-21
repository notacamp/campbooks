# frozen_string_literal: true

module Api
  module App
    module Settings
      # Email signature.
      class SignatureSerializer
        def initialize(signature)
          @sig = signature
        end

        def as_json(*)
          {
            id: @sig.id,
            name: @sig.name,
            content: @sig.content,
            is_default: @sig.is_default,
            email_account_ids: @sig.email_account_ids,
            created_at: @sig.created_at
          }
        end
      end
    end
  end
end

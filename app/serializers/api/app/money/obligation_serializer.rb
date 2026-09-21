# frozen_string_literal: true

module Api
  module App
    module Money
      # Serializes one Money::Obligation (a value object from Money::Ledger) into a
      # plain hash for the SPA's obligations list.
      class ObligationSerializer
        def initialize(obligation)
          @ob = obligation
        end

        def as_json
          {
            id:            @ob.id,
            direction:     @ob.direction,
            counterpart:   @ob.counterpart,
            what:          @ob.what,
            amount_cents:  @ob.amount_cents,
            currency:      @ob.currency,
            anchor_on:     @ob.anchor_on&.iso8601,
            status:        @ob.status,
            settled_on:    @ob.settled_on&.iso8601,
            settled_via:   @ob.settled_via,
            document_id:   @ob.document&.id,
            document_title: @ob.document&.display_title,
            statement_label: @ob.statement_label,
            actions:       Array(@ob.actions)
          }
        end
      end
    end
  end
end

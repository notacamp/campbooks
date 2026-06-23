module Emails
  module SyncStrategies
    # Tally returned by a strategy run, folded onto the EmailScanLog by the engine.
    Result = Data.define(:found, :created, :reconciled) do
      def self.empty = new(found: 0, created: 0, reconciled: 0)

      def add(outcome)
        case outcome
        when :created    then Result.new(found: found + 1, created: created + 1, reconciled: reconciled)
        when :reconciled then Result.new(found: found + 1, created: created, reconciled: reconciled + 1)
        else self
        end
      end

      def merge(other)
        Result.new(found: found + other.found, created: created + other.created,
                   reconciled: reconciled + other.reconciled)
      end
    end

    # Resolve the per-vendor sync strategy for an account. Vendor capability lives
    # in the strategy, not the engine: Gmail and Microsoft do true delta, Zoho —
    # which has no change feed — windows new mail and reconciles on a slower pass.
    def self.for(account)
      case account.provider.to_sym
      when :google    then Google.new(account)
      when :microsoft then Microsoft.new(account)
      else                 Zoho.new(account)
      end
    end
  end
end

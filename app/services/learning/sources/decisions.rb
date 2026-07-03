module Learning
  module Sources
    # Reads human verdicts from the shared learning_decisions table for one domain.
    # Bulk: a single windowed query buckets every decision into per-tier
    # { key => { label => count } } tallies, so a page of many lookups costs one
    # round-trip. Used by domains whose suggestions are ephemeral (Skim clusters,
    # feed tag-suggestions) and thus have no durable verdict record of their own.
    #
    # Each tier maps to a learning_decisions column with an optional normalizer that
    # is applied identically when bucketing a row and when looking up a caller key,
    # so record-time and lookup-time always agree (e.g. domains match case-insensitively).
    class Decisions < Base
      Tier = Data.define(:name, :column, :normalize) do
        def key_for(value)
          normalize ? normalize.call(value) : value
        end
      end

      def self.tier(name, column, normalize: nil)
        Tier.new(name: name, column: column, normalize: normalize)
      end

      # scope: a Hash of extra AR conditions scoping ownership, e.g. { user_id: id }
      #        or { workspace_id: id }.
      def initialize(domain:, scope:, tiers:, window: nil, now: Time.current)
        @domain = domain
        @scope  = scope
        @tiers  = tiers
        @window = window
        @now    = now
      end

      def signal_cascade = @tiers.map(&:name)

      def tally_for(signal, **context)
        tier = @tiers.find { |t| t.name == signal }
        return nil unless tier

        key = tier.key_for(context[signal])
        return nil if key.nil? || key == ""

        tallies.dig(tier.name, key)
      end

      private

      # Preload every in-window decision once and bucket it into per-tier tallies,
      # so each subsequent lookup is a hash hit (mirrors the old SkimActionMemory).
      def tallies
        @tallies ||= begin
          columns = @tiers.map(&:column).uniq
          acc = {}
          relation.pluck(*columns, :label).each do |row|
            *values, label = row
            by_column = columns.zip(values).to_h
            @tiers.each do |tier|
              key = tier.key_for(by_column[tier.column])
              next if key.nil? || key == ""

              ((acc[tier.name] ||= {})[key] ||= Hash.new(0))[label] += 1
            end
          end
          acc
        end
      end

      def relation
        rel = LearningDecision.where(domain: @domain, **@scope)
        rel = rel.where(created_at: (@now - @window)..) if @window
        rel
      end
    end
  end
end

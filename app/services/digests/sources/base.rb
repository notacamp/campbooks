# frozen_string_literal: true

module Digests
  module Sources
    # Abstract base for a digest source. Each concrete subclass gathers items from
    # one domain (emails, calendar, tasks, reminders, or documents) and wraps them
    # in Digests::Item value objects capped at MAX_ITEMS.
    class Base
      MAX_ITEMS = 50

      # @param digest [ScheduledDigest]
      # @param source_config [Hash] the source-specific config hash from digest.config["sources"]
      def initialize(digest, source_config)
        @digest = digest
        @source_config = source_config
      end

      # Return an Array<Digests::Item>, capped at MAX_ITEMS, sensibly ordered.
      # @param period [Range<Time>] — lookback: [period_start, period_end];
      #                              lookahead: [period_end, period_end + window]
      def items(period)
        raise NotImplementedError
      end

      # :lookback or :lookahead — determines how the generator computes the window.
      def self.direction
        raise NotImplementedError
      end

      private

      attr_reader :digest, :source_config

      def user
        digest.user
      end

      def workspace
        digest.workspace
      end

      def truncate(str, length: 200)
        return "" if str.blank?

        str = str.to_s.strip.gsub(/\s+/, " ")
        str.length > length ? "#{str[0, length - 1]}…" : str
      end
    end
  end
end

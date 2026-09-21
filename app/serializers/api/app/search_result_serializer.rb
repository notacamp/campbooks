# frozen_string_literal: true

module Api
  module App
    # Serializes GlobalSearch results (array of uniform hashes) into a grouped
    # structure the SPA's Cmd+K command palette can render directly.
    class SearchResultSerializer
      def initialize(results)
        @results = results
      end

      def as_json
        {
          results:  @results,
          grouped:  grouped_results,
          total:    @results.size
        }
      end

      private

      def grouped_results
        @results.group_by { |r| r[:type] }.map do |type, items|
          { type: type, items: items }
        end
      end
    end
  end
end

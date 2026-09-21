# frozen_string_literal: true

module Api
  module App
    # Serializes the Today surface payload produced by Today::Aggregator.
    # Wraps the aggregator's pre-built hashes into the locked frontend shape:
    #
    #   { greeting:, needs_you:, coming_up:, handled: }
    #
    # The needs_you items already carry their action arrays; this serializer
    # only adds the greeting (locale-aware name + date + one-line brief).
    class TodaySerializer
      def initialize(user:, result:)
        @user   = user
        @result = result
      end

      def as_json
        needs_you  = @result[:needs_you]
        coming_up  = @result[:coming_up]

        {
          greeting:  build_greeting(needs_you, coming_up),
          needs_you: needs_you,
          coming_up: coming_up,
          handled:   @result[:handled]
        }
      end

      private

      def build_greeting(needs_you, coming_up)
        first_name = @user.name.split(" ").first.presence || @user.name
        needs_count = needs_you.size
        up_count    = coming_up.size

        brief = case [ needs_count, up_count ]
        in [ 0, 0 ] then "Nothing urgent today."
        in [ 0, _ ] then "#{up_count} #{up_count == 1 ? "thing" : "things"} coming up."
        in [ 1, 0 ] then "1 thing needs you."
        in [ 1, _ ] then "1 thing needs you, #{up_count} coming up."
        in [ n, 0 ] then "#{n} things need you."
        else              "#{needs_count} things need you, #{up_count} coming up."
        end

        {
          name:  first_name,
          date:  ::Date.current.iso8601,
          brief: brief
        }
      end
    end
  end
end

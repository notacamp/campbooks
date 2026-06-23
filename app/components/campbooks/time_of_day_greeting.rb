# frozen_string_literal: true

module Campbooks
  # The home page's time-of-day greeting: a tinted time-of-day glyph beside a
  # warm "Morning, Alex." headline, with the orienting subtitle stacked
  # underneath.
  #
  # Time-of-day is read from the VISITOR'S device clock, not the server's. The
  # server renders a best-effort default from its own time (often UTC in
  # production, so wrong for most people); the `local-greeting` Stimulus
  # controller then corrects both the headline and the glyph on connect using
  # `new Date()`. No IP lookup, no timezone cookie, no permission prompt — the
  # device clock is the most accurate read of "the user's local time".
  #
  # The hour thresholds in #default_bucket MUST stay in sync with
  # local_greeting_controller.js#bucketFor.
  class TimeOfDayGreeting < Campbooks::Base
    # bucket => accent color. The three daytime buckets share the warm Ember
    # accent; night gets a cool indigo so the moon reads as night at a glance.
    BUCKETS = {
      morning: "var(--ember-solid)",
      afternoon: "var(--ember-solid)",
      evening: "var(--ember-solid)",
      night: "#818cf8"
    }.freeze

    # Lucide-style glyphs: sunrise, sun, sunset, moon-star.
    ICONS = {
      morning: %(<path d="M12 2v8"/><path d="m4.93 10.93 1.41 1.41"/><path d="M2 18h2"/><path d="M20 18h2"/><path d="m19.07 10.93-1.41 1.41"/><path d="M22 22H2"/><path d="m8 6 4-4 4 4"/><path d="M16 18a4 4 0 0 0-8 0"/>),
      afternoon: %(<circle cx="12" cy="12" r="4"/><path d="M12 2v2"/><path d="M12 20v2"/><path d="m4.93 4.93 1.41 1.41"/><path d="m17.66 17.66 1.41 1.41"/><path d="M2 12h2"/><path d="M20 12h2"/><path d="m6.34 17.66-1.41 1.41"/><path d="m19.07 4.93-1.41 1.41"/>),
      evening: %(<path d="M12 10V2"/><path d="m4.93 10.93 1.41 1.41"/><path d="M2 18h2"/><path d="M20 18h2"/><path d="m19.07 10.93-1.41 1.41"/><path d="M22 22H2"/><path d="m16 6-4 4-4-4"/><path d="M16 18a4 4 0 0 0-8 0"/>),
      night: %(<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/><path d="M20 3v4"/><path d="M22 5h-4"/>)
    }.freeze

    # @param name [String] the first name to greet
    # @param subtitle [String, nil] the orienting line under the headline
    # @param now [Time] server-side time for the no-JS / first-paint default
    def initialize(name:, subtitle: nil, now: Time.current)
      @name = name
      @subtitle = subtitle
      @now = now
    end

    def view_template
      div(
        class: "flex items-start gap-3",
        data: {
          controller: "local-greeting",
          local_greeting_greetings_value: greetings.to_json
        }
      ) do
        BUCKETS.each_key { |bucket| icon_tile(bucket) }

        div do
          h1(
            class: "text-xl font-semibold leading-9 tracking-tight text-foreground",
            data: { local_greeting_target: "text" }
          ) { greetings[default_bucket] }
          if @subtitle
            p(class: "mt-1 text-[13.5px] text-muted-foreground") { @subtitle }
          end
        end
      end
    end

    private

    # All four headlines, pre-interpolated, handed to the controller so it can
    # swap to the device-local one without another server round-trip.
    def greetings
      @greetings ||= BUCKETS.keys.index_with { |bucket| t(".greeting.#{bucket}", name: @name) }
    end

    # A soft tinted tile holding the glyph. Every bucket's tile is rendered up
    # front; all but the server default carry `hidden`, and the controller
    # reveals the device-local one (and hides the rest) on connect. `.hidden`
    # beats `.flex` in the compiled CSS, so keeping both is the standard
    # "hidden until toggled" shape (see Campbooks::Base.class_names).
    def icon_tile(bucket)
      color = BUCKETS[bucket]
      classes = "flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-xl"
      classes += " hidden" unless bucket == default_bucket

      div(
        class: classes,
        style: "background-color: color-mix(in oklab, #{color} 14%, transparent); color: #{color}",
        data: { local_greeting_target: "icon", bucket: bucket },
        aria_hidden: "true"
      ) do
        svg(
          class: "h-5 w-5", fill: "none", stroke: "currentColor", stroke_width: "2",
          stroke_linecap: "round", stroke_linejoin: "round", viewBox: "0 0 24 24"
        ) { raw(safe(ICONS[bucket])) }
      end
    end

    # Keep these thresholds identical to local_greeting_controller.js#bucketFor.
    def default_bucket
      @default_bucket ||= case @now.hour
      when 5...12 then :morning
      when 12...17 then :afternoon
      when 17...22 then :evening
      else :night
      end
    end
  end
end

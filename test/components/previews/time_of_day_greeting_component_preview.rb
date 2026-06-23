# frozen_string_literal: true

# The server renders a best-effort greeting from `now:`; on the live page the
# local-greeting Stimulus controller corrects it to the visitor's device clock.
# Each example below pins a different hour so all four time-of-day buckets
# (icon + headline) are visible here in Lookbook.
class TimeOfDayGreetingComponentPreview < ViewComponent::Preview
  def morning
    render(Campbooks::TimeOfDayGreeting.new(
      name: "Alex",
      subtitle: "8 things need you — Scout stacked them. A quick skim and you're done.",
      now: Time.current.change(hour: 8)
    ))
  end

  def afternoon
    render(Campbooks::TimeOfDayGreeting.new(
      name: "Alex",
      subtitle: "Just one thing needs you — Scout filed the rest.",
      now: Time.current.change(hour: 14)
    ))
  end

  def evening
    render(Campbooks::TimeOfDayGreeting.new(
      name: "Alex",
      subtitle: "47 things need you. Scout stacked them so you can skim, not slog.",
      now: Time.current.change(hour: 19)
    ))
  end

  def night
    render(Campbooks::TimeOfDayGreeting.new(
      name: "Alex",
      subtitle: "All clear — Scout's on watch so it stays that way.",
      now: Time.current.change(hour: 23)
    ))
  end

  # Headline only, no subtitle — confirms the glyph stays vertically centred
  # against a single line.
  def without_subtitle
    render(Campbooks::TimeOfDayGreeting.new(name: "Alex", now: Time.current.change(hour: 8)))
  end
end

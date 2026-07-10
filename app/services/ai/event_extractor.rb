module Ai
  # Best-effort extraction of calendar-event details from an email, used to seed
  # the "create event from this email" action. Heuristic for v1 so it runs
  # instantly inside the synchronous action (no model round-trip); the interface
  # leaves room to swap in an AI backend later. The user can always refine the
  # event afterwards on the calendar.
  class EventExtractor
    Result = Struct.new(:title, :start_at, :end_at, :all_day, :location, keyword_init: true)

    def initialize(email_message)
      @email = email_message
    end

    def extract
      start_at = guess_start
      Result.new(title: title, start_at: start_at, end_at: start_at + 3600, all_day: false, location: guess_location)
    end

    # Returns true when the email text contains an explicit time mention (e.g.
    # "2:30 PM", "3pm", "14:00"). Used as a cheap gate before showing the inline
    # event-draft block on the email page — avoids surfacing the block on every
    # email just because the extractor always returns a default.
    # Quoted reply chains and "On … wrote:" attribution lines are excluded:
    # their timestamps would otherwise trigger the block on every threaded reply.
    def has_time_proposal?
      scan = text.lines.reject { |l| l.lstrip.start_with?(">") || l.match?(/\bwrote:\s*$/i) }.join
      scan.match?(/\b\d{1,2}:\d{2}\s*(?:am|pm)?\b/i) ||
        scan.match?(/\b\d{1,2}\s*(?:am|pm)\b/i)
    end

    private

    def text
      @text ||= [ @email.subject, @email.body.to_s.truncate(4000) ].compact.join("\n")
    end

    def title
      @email.subject.to_s.sub(/^(?:re|fwd?|fw)\s*:\s*/i, "").strip.presence || "Event"
    end

    # Default to the day after the email arrived, at 09:00 — anchored to the email's
    # own date, not "now", so drafting an event from a months-old email doesn't
    # silently land it in the current year. "today" in the text pins it to the
    # email's day. The user refines the date on the calendar afterwards.
    def guess_start
      base = @email.received_at&.in_time_zone&.to_date || Date.current
      day = base + 1
      day = base if text.match?(/\btoday\b/i)
      hour, min = guess_time
      Time.zone.local(day.year, day.month, day.day, hour, min)
    end

    def guess_time
      if (m = text.match(/\b(\d{1,2}):(\d{2})\s*(am|pm)?\b/i))
        h = m[1].to_i
        h += 12 if m[3]&.downcase == "pm" && h < 12
        h = 0 if m[3]&.downcase == "am" && h == 12
        [ h % 24, m[2].to_i ]
      elsif (m = text.match(/\b(\d{1,2})\s*(am|pm)\b/i))
        h = m[1].to_i
        h += 12 if m[2].downcase == "pm" && h < 12
        h = 0 if m[2].downcase == "am" && h == 12
        [ h % 24, 0 ]
      else
        [ 9, 0 ]
      end
    end

    def guess_location
      if (m = text.match(/(?:location|where|venue)\s*[:\-]\s*(.+)/i))
        m[1].strip.truncate(120)
      elsif (m = text.match(%r{https?://[\w.-]*(?:zoom\.us|meet\.google\.com|teams\.microsoft\.com)\S*}i))
        m[0]
      end
    end
  end
end

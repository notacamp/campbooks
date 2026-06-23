# frozen_string_literal: true

module Home
  # Builds the home feed from real email data: the threads carrying a live Scout
  # action prompt (`with_ai_todos`) become FeedCard hashes — Scout's read +
  # a suggested action per thread. Rings come separately from the lazy Skim tray.
  class FeedBuilder
    LIMIT = 12

    # machine tool key (ai_suggested_actions[0].tool) → human button label
    TOOL_LABELS = {
      "draft_reply" => "Draft reply", "send_reply" => "Send reply",
      "archive" => "Archive & file", "add_tag" => "File it", "snooze" => "Snooze"
    }.freeze

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    def call
      messages.map { |m| card_for(m) }
    end

    private

    def messages
      EmailMessage
        .accessible_to(@user)
        .with_ai_todos
        .where(skimmed_at: nil)
        .limit(LIMIT)
    end

    def card_for(m)
      name = display_name(m.from_address)
      {
        type: :card,
        email_id: m.id,
        initials: initials(name),
        sender: name,
        time: relative_time(m.received_at),
        subject: clean_subject(m.subject),
        excerpt: (m.ai_summary.presence || m.summary.presence || "").to_s.truncate(220),
        scout: (m.ai_action_prompt.presence || m.ai_summary.presence || "I've read this for you.").to_s,
        prime: prime_label(m),
        tag: m.category.presence&.titleize,
        attachment: (m.has_attachment ? "Attachment" : nil),
        priority: (m.ai_priority.to_s == "high" || m.pinned_at.present?)
      }
    end

    def prime_label(m)
      tool = m.ai_suggested_actions&.first&.dig("tool")
      TOOL_LABELS[tool] || "Open & reply"
    end

    # "Display Name <addr@x.com>" → "Display Name"; bare addr → humanized local part
    def display_name(from)
      return "Unknown sender" if from.blank?
      if from =~ /\A\s*"?([^"<]+?)"?\s*<.*>\s*\z/
        Regexp.last_match(1).strip
      else
        from.split("@").first.to_s.tr("._", " ").split.map(&:capitalize).join(" ").presence || from
      end
    end

    def initials(name)
      name.to_s.split(/\s+/).first(2).filter_map { |p| p[0] }.join.upcase.presence || "?"
    end

    def clean_subject(subject)
      subject.to_s.sub(/\A((re|fwd?):\s*)+/i, "").strip.presence || "(no subject)"
    end

    def relative_time(t)
      return "" if t.nil?
      if t.to_date == @now.to_date then t.strftime("%-l:%M %p")
      elsif t.to_date == @now.to_date - 1 then "Yesterday"
      elsif t > @now - 7.days then t.strftime("%a")
      else t.strftime("%b %-d")
      end
    end
  end
end

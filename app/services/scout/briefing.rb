# frozen_string_literal: true

module Scout
  # Builds the data for Scout's proactive empty-state briefing: a time-aware
  # greeting, a one-line read on the state of the inbox, a handful of live
  # counts (rendered as one-tap prompts), and starter questions.
  #
  # Counts mirror how Scout already reports numbers (Tools::SystemStats) — i.e.
  # across the visible data set — and every count is fail-safe so a briefing
  # never breaks the chat page.
  class Briefing
    def self.for(user)
      new(user).build
    end

    def initialize(user)
      @user = user
    end

    def build
      {
        greeting: greeting,
        subtitle: subtitle,
        stats: stats,
        suggestions: suggestions
      }
    end

    private

    def greeting
      name = @user&.name.to_s.split(/\s+/).first
      if name.present?
        I18n.t("scout.briefing.greeting.with_name", time_of_day: time_of_day, name: name)
      else
        time_of_day
      end
    end

    def time_of_day
      hour = Time.current.hour
      case hour
      when 5...12  then I18n.t("scout.briefing.greeting.morning")
      when 12...18 then I18n.t("scout.briefing.greeting.afternoon")
      else I18n.t("scout.briefing.greeting.evening")
      end
    end

    def subtitle
      if counts[:high_priority].positive?
        I18n.t("scout.briefing.subtitle.urgent", count: counts[:high_priority])
      elsif counts[:unread].positive?
        I18n.t("scout.briefing.subtitle.has_unread")
      else
        I18n.t("scout.briefing.subtitle.all_caught_up")
      end
    end

    # Curated, ordered, deduped — show the few that drive action, max 4.
    def stats
      candidates = [
        { key: :high_priority, value: counts[:high_priority],
          label: I18n.t("scout.briefing.stats.high_priority.label"), icon: :flag,
          tone: :amber, prompt: I18n.t("scout.briefing.stats.high_priority.prompt"), always: true },
        { key: :unread, value: counts[:unread],
          label: I18n.t("scout.briefing.stats.unread.label"), icon: :inbox,
          tone: :accent, prompt: I18n.t("scout.briefing.stats.unread.prompt"), always: true },
        { key: :waiting_on_reply, value: counts[:waiting_on_reply],
          label: I18n.t("scout.briefing.stats.waiting_on_reply.label"), icon: :clock,
          tone: :amber, prompt: I18n.t("scout.briefing.stats.waiting_on_reply.prompt") },
        { key: :docs_review, value: counts[:docs_review],
          label: I18n.t("scout.briefing.stats.docs_review.label"), icon: :document,
          tone: :default, prompt: I18n.t("scout.briefing.stats.docs_review.prompt") },
        { key: :this_week, value: counts[:this_week],
          label: I18n.t("scout.briefing.stats.this_week.label"), icon: :clock,
          tone: :default, prompt: I18n.t("scout.briefing.stats.this_week.prompt") }
      ]

      candidates.select { |c| c[:always] || c[:value].to_i.positive? }
        .first(4)
        .map { |c| c.slice(:value, :label, :icon, :tone, :prompt) }
    end

    def suggestions
      [
        I18n.t("scout.briefing.suggestions.attention_today"),
        I18n.t("scout.briefing.suggestions.recent_invoices"),
        I18n.t("scout.briefing.suggestions.newsletters"),
        I18n.t("scout.briefing.suggestions.week_summary")
      ]
    end

    def counts
      emails = EmailMessage.accessible_to(@user)
      documents = @user&.workspace&.documents || Document.none

      @counts ||= {
        unread: safe { emails.where(read: false).count },
        high_priority: safe { emails.where(ai_priority: :high).count },
        docs_review: safe { documents.needs_review.count },
        this_week: safe { emails.where("received_at >= ?", 1.week.ago).count },
        # AI-free: the threads the user sent last and is still waiting to hear back
        # on (Emails::AwaitingReply). Fail-safe to 0 so the briefing never breaks.
        waiting_on_reply: safe { Emails::AwaitingReply.new(@user).count }
      }
    end

    def safe
      yield
    rescue => e
      Rails.logger.warn("[Scout::Briefing] count failed: #{e.message}")
      0
    end
  end
end

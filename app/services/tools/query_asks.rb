# frozen_string_literal: true

module Tools
  # Scout's read tool for "what do I owe people?". Returns the acting user's live
  # asks (for_user — things people asked them to do, or they promised), with the
  # same filters the model reasons over (status / undated / due-within / handed to
  # others) plus a small deck of `cards` (kind "ask") the chat renders as tappable
  # rows. Gated by Features.tasks?; fails closed to an empty result with no user.
  class QueryAsks
    include ::Rails.application.routes.url_helpers

    DEFAULT_LIMIT = 20
    MAX_LIMIT = 50
    CARD_LIMIT = 10

    def self.call(args = {})
      new(args).call
    end

    def initialize(args)
      @args = (args || {}).transform_keys(&:to_s)
      @user = Current.user
    end

    def call
      return empty unless @user && Features.tasks?

      records = scope.to_a
      I18n.with_locale(@user.locale.presence || I18n.default_locale) do
        asks = records.map { |task| ask_hash(task) }
        { count: asks.size, asks: asks, cards: cards_for(records.first(CARD_LIMIT)) }
      end
    end

    private

    def empty
      { count: 0, asks: [], cards: [] }
    end

    def scope
      base = handed? ? handed_scope : Task.for_user(@user).live
      base = apply_status(base)
      base = base.undated if boolean(@args["undated"])
      base = apply_due_within(base)
      # preload (not includes): source is polymorphic, and the handed base already
      # joins task_assignments — includes would be promoted to an eager_load JOIN
      # and raise EagerLoadPolymorphicError on :source.
      base.preload(:source, :task_assignments)
          .order(Arel.sql("due_at ASC NULLS LAST"), created_at: :desc)
          .limit(limit)
    end

    # Asks I handed to a teammate (assigned_by me, to someone else) — these leave
    # for_user, so they need their own base when the model asks about them.
    def handed_scope
      Task.where(workspace_id: @user.workspace_id).live
          .joins(:task_assignments)
          .where(task_assignments: { assigned_by_id: @user.id })
          .where.not(task_assignments: { user_id: @user.id })
    end

    # Default (and "all") is the live set = open + suggested, which `.live` already
    # gives; "open" narrows to accepted asks, "suggested" to Scout's proposals.
    def apply_status(base)
      case @args["status"].to_s
      when "open"      then base.where(status: Task::ACTIVE_STATUSES)
      when "suggested" then base.where(status: :suggested)
      else base
      end
    end

    def apply_due_within(base)
      days = @args["due_within_days"]
      return base if days.nil?

      n = days.to_i
      return base if n.negative?

      base.where.not(due_at: nil).where(due_at: ..(Time.current + n.days))
    end

    def limit
      n = @args["limit"].to_i
      return DEFAULT_LIMIT if n <= 0

      [ n, MAX_LIMIT ].min
    end

    def handed?
      boolean(@args["handed"])
    end

    def boolean(value)
      ActiveModel::Type::Boolean.new.cast(value)
    end

    def ask_hash(task)
      {
        id: task.id,
        title: task.title,
        status: task.status,
        due_at: task.due_at&.iso8601,
        held_at: task.held_block&.start_at&.iso8601,
        counterpart: counterpart_name(task),
        handed_to: task.handed_to&.name,
        source_path: source_path(task)
      }
    end

    def cards_for(records)
      records.map do |task|
        {
          "id" => task.id,
          "title" => task.title,
          "meta" => card_meta(task),
          "path" => source_path(task),
          "kind" => "ask"
        }
      end
    end

    def card_meta(task)
      parts = [ due_label(task) ]
      block = task.held_block
      parts << I18n.t("scout.ask_cards.held", when: when_label(block.start_at)) if block
      if (to = task.handed_to) && to != @user
        parts << I18n.t("scout.ask_cards.handed_to", name: first_name(to))
      elsif (name = counterpart_name(task))
        parts << I18n.t("scout.ask_cards.from", name: name)
      end
      parts.compact.join(" · ")
    end

    def due_label(task)
      return I18n.t("scout.ask_cards.no_date") if task.due_at.nil?

      due = task.due_at.in_time_zone(zone).to_date
      case due <=> today
      when -1 then I18n.t("scout.ask_cards.overdue")
      when 0  then I18n.t("scout.ask_cards.due_today")
      else I18n.t("scout.ask_cards.by", date: I18n.l(due, format: :long))
      end
    end

    def counterpart_name(task)
      email = task.source_email
      return nil unless email

      email.contact&.person&.name.presence ||
        Emails::SenderName.first_name(email.from_address).presence ||
        email.from_address
    end

    def source_path(task)
      email = task.source_email
      email ? email_message_path(email) : nil
    end

    def first_name(user)
      user.name.to_s.split(/\s+/).first.presence || user.name.presence || user.email_address.to_s
    end

    def when_label(time)
      I18n.l(time.in_time_zone(zone), format: "%a %H:%M")
    end

    def zone
      @zone ||= @user.effective_time_zone
    end

    def today
      @today ||= Time.current.in_time_zone(zone).to_date
    end
  end
end

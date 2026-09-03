# frozen_string_literal: true

module People
  # Scout's one-liner about where things stand with a counterpart — the third
  # line on every People row and the pinned "where things stand" block. Pure over
  # the reply state, AI reads, and profile the app already computes; no new AI.
  #
  # Priority ladder for a person (first match wins):
  #   1. You owe them a reply — their last message is unanswered past the grace
  #      window → "Waiting on your reply for N days." (needs you)
  #   2. They owe you — you had the last word and the nudge is due
  #      (Emails::AwaitingReply#due) → "No reply to <subject> for N days. Nudge?"
  #      (needs you)
  #   3. The latest message carries a Scout action prompt → that prompt.
  #   4. Scout's profile summary → its first sentence.
  #   5. Fallback → "Last exchange <date>."
  #
  # An organization composes its people's standings into at most two sentences,
  # and "needs you" when any of them does.
  #
  # NOTE: the mock's priority-1 copy string ("Waiting on your reply…") is the
  # you-owe-them case; Emails::AwaitingReply#due is the *nudge* set (you owe
  # nothing, they've gone quiet), so it drives priority 2 here. Mapping the copy
  # to the branch it describes keeps the surface honest and matches the mock.
  class Standing
    Result = Data.define(:text, :needs_you, :thread_id, :overdue_days) do
      def self.none = new(text: nil, needs_you: false, thread_id: nil, overdue_days: 0)
      def present? = text.present?
    end

    GRACE = EmailThread::AWAITING_REPLY_GRACE
    ORG_PEOPLE_SAMPLE = 5

    def self.for_person(person, user:, now: Time.current)
      new(user, now: now).person(person)
    end

    def self.for_organization(organization, user:, now: Time.current)
      new(user, now: now).organization(organization)
    end

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    def person(person)
      threads = person_threads(person)

      if (thread = you_owe_thread(threads))
        days = days_since(thread.last_inbound_at)
        return result(t("you_owe", count: days), needs_you: true, thread: thread, days: days)
      end

      if (thread = nudge_thread(threads))
        days = days_since(thread.last_outbound_at)
        subject = thread.display_subject.to_s.strip
        subject = subject.present? ? subject.truncate(48) : t("your_last_message")
        return result(t("awaiting_them", subject: subject, count: days), needs_you: true, thread: thread, days: days)
      end

      latest = latest_inbound_message(person)
      if latest&.ai_action_prompt.to_s.strip.present?
        return result(latest.ai_action_prompt.strip, thread: latest.email_thread)
      end

      if (summary = profile_summary(person)).present?
        return result(first_sentence(summary), thread: latest&.email_thread)
      end

      if (last = person.last_email_at)
        return result(t("last_exchange", date: I18n.l(last.to_date, format: :short)), thread: latest&.email_thread)
      end

      Result.none
    end

    def organization(organization)
      standings = organization_person_standings(organization).select(&:present?)
      return Result.none if standings.empty?

      needing = standings.select(&:needs_you)
      chosen = (needing.sort_by { |s| -s.overdue_days } + (standings - needing)).first(2)

      Result.new(
        text: chosen.map(&:text).join(" "),
        needs_you: needing.any?,
        thread_id: chosen.first&.thread_id,
        overdue_days: (needing.map(&:overdue_days).max || 0)
      )
    end

    private

    def t(key, **opts) = I18n.t("people.standing.#{key}", **opts)

    def result(text, thread: nil, needs_you: false, days: 0)
      Result.new(text: text, needs_you: needs_you, thread_id: thread&.id, overdue_days: days)
    end

    # The counterpart's inbox threads on mailboxes the user can read, messages
    # preloaded (the reply-state columns are read per thread).
    def person_threads(person)
      contact_ids = person.contacts.ids
      return [] if contact_ids.empty? || readable_account_ids.empty?

      scope = EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil)
      scope = scope.where(provider_folder_id: inbox_folder_ids) if inbox_folder_ids.present?
      thread_ids = scope.distinct.pluck(:email_thread_id)
      return [] if thread_ids.empty?

      EmailThread.where(id: thread_ids, email_account_id: readable_account_ids)
                 .includes(:email_messages)
                 .to_a
    end

    # You owe a reply: they had the last word (thread does not hold_last_word) and
    # it has sat past the grace window. Newest such thread wins.
    def you_owe_thread(threads)
      threads.select { |thread|
        !thread.holds_last_word? &&
          thread.last_inbound_at.present? &&
          thread.last_inbound_at <= @now - GRACE
      }.max_by(&:last_inbound_at)
    end

    # They owe you and the nudge is due (the AwaitingReply proactive subset),
    # restricted to this counterpart's threads. Longest-silent (oldest outbound) wins.
    def nudge_thread(threads)
      ids = threads.map(&:id).to_set
      due = awaiting_reply.due.select { |thread| ids.include?(thread.id) }
      due.min_by { |thread| thread.last_outbound_at || @now }
    end

    def latest_inbound_message(person)
      contact_ids = person.contacts.ids
      return nil if contact_ids.empty?

      EmailMessage.where(contact_id: contact_ids)
                  .accessible_to(@user)
                  .order(received_at: :desc)
                  .first
    end

    def profile_summary(person)
      person.read_attribute(:context_summary).presence ||
        person.contacts.where.not(context_summary: [ nil, "" ])
              .order(email_count: :desc).limit(1).pick(:context_summary)
    end

    def organization_person_standings(organization)
      people = organization.active_people.includes(:contacts)
                           .select { |person| person.contacts.any?(&:kind_person?) }
                           .sort_by { |person| person.last_email_at || Time.at(0) }
                           .reverse
                           .first(ORG_PEOPLE_SAMPLE)
      people.map { |person| person(person) }
    end

    def first_sentence(text)
      text.to_s.strip.split(/(?<=[.!?])\s+/).first.to_s
    end

    def days_since(time)
      return 0 if time.blank?

      [ ((@now - time) / 1.day).floor, 0 ].max
    end

    def awaiting_reply
      @awaiting_reply ||= Emails::AwaitingReply.new(@user, now: @now)
    end

    def readable_account_ids
      @readable_account_ids ||= @user&.readable_email_accounts&.ids || []
    end

    def inbox_folder_ids
      @inbox_folder_ids ||= Emails::InboxFolders.ids_for(@user&.readable_email_accounts&.to_a || [])
    end
  end
end

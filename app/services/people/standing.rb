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
    # `kind` names the rung that produced the text (:you_owe, :nudge, :prompt,
    # :summary, :last_exchange, :none) so a ranker (People::Priority) can weigh a
    # reply you owe differently from a nudge you could send without parsing copy.
    Result = Data.define(:text, :needs_you, :thread_id, :overdue_days, :kind) do
      def initialize(text:, needs_you: false, thread_id: nil, overdue_days: 0, kind: :none) = super
      def self.none = new(text: nil)
      def present? = text.present?
    end

    GRACE = EmailThread::AWAITING_REPLY_GRACE
    # An unanswered message older than this is no longer "waiting on your reply" —
    # silence was the triage. Mirrors the nudge horizon on the other side.
    STALE_AFTER = Emails::AwaitingReply::MAX_NUDGE_AGE
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

    # Bulk-load everything the per-record standings need so a whole People list
    # resolves in a handful of queries instead of a few per counterpart. Without
    # this each `person`/`organization` call falls back to its own queries (the
    # single-record path used by the detail pages); with it they read from primed
    # maps. Idempotent and additive — call it again with more people/orgs and only
    # the newly-seen records are loaded. The decision logic is unchanged; only the
    # data source moves from per-record to batched.
    def prime(people: [], organizations: [])
      @primed = true
      @threads_by_person ||= {}
      @latest_inbound_by_person ||= {}
      @primed_person_ids ||= Set.new
      @org_sample ||= {}

      new_orgs = organizations.reject { |org| @org_sample.key?(org.id) }
      if new_orgs.any?
        active = load_org_active_people(new_orgs)
        new_orgs.each { |org| @org_sample[org.id] = org_sample_from(active[org.id] || []) }
      end

      sampled = @org_sample.values_at(*organizations.map(&:id)).compact.flatten
      to_prime = (people + sampled).reject { |person| @primed_person_ids.include?(person.id) }.uniq(&:id)
      prime_person_data(to_prime) if to_prime.any?
      self
    end

    def person(person)
      threads = person_threads(person)

      if (thread = you_owe_thread(threads))
        days = days_since(thread.last_inbound_at)
        return result(I18n.t("people.standing.you_owe", count: days), needs_you: true, thread: thread, days: days, kind: :you_owe)
      end

      if (thread = nudge_thread(threads))
        days = days_since(thread.last_outbound_at)
        subject = thread.display_subject.to_s.strip
        subject = subject.present? ? subject.truncate(48) : I18n.t("people.standing.your_last_message")
        return result(I18n.t("people.standing.awaiting_them", subject: subject, count: days), needs_you: true, thread: thread, days: days, kind: :nudge)
      end

      latest = latest_inbound_message(person)
      if latest&.ai_action_prompt.to_s.strip.present?
        return result(latest.ai_action_prompt.strip, thread: latest.email_thread, kind: :prompt)
      end

      if (summary = profile_summary(person)).present?
        return result(first_sentence(summary), thread: latest&.email_thread, kind: :summary)
      end

      if (last = person.last_email_at)
        return result(I18n.t("people.standing.last_exchange", date: I18n.l(last.to_date, format: :short)), thread: latest&.email_thread, kind: :last_exchange)
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
        overdue_days: (needing.map(&:overdue_days).max || 0),
        kind: chosen.first&.kind || :none
      )
    end

    # The rows behind a standing, for callers that rank or explain the list
    # (People::Priority): a person's inbox threads (messages preloaded), their
    # newest inbound message, and the people an organization's standing is
    # composed from. Primed-or-live, exactly like the standing itself.
    def threads_for(person) = person_threads(person)
    def latest_inbound_for(person) = latest_inbound_message(person)

    def sampled_people_for(organization)
      return @org_sample[organization.id] if @primed && @org_sample&.key?(organization.id)

      org_sample_from(organization.active_people.includes(:contacts).to_a)
    end

    private

    def result(text, thread: nil, needs_you: false, days: 0, kind: :none)
      Result.new(text: text, needs_you: needs_you, thread_id: thread&.id, overdue_days: days, kind: kind)
    end

    # The counterpart's inbox threads on mailboxes the user can read, messages
    # preloaded (the reply-state columns are read per thread). Served from the
    # primed batch when this instance was primed for this person (the list path),
    # else loaded on the spot (the single-record detail path).
    def person_threads(person)
      return @threads_by_person[person.id] || [] if primed?(person)

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

    # You owe a reply: they had the last word (thread does not hold_last_word), it
    # has sat past the grace window without going stale, and the unanswered
    # message is one you'd actually answer (#reply_owed?). Newest such thread wins.
    def you_owe_thread(threads)
      threads.select { |thread|
        !thread.holds_last_word? &&
          thread.last_inbound_at.present? &&
          thread.last_inbound_at <= @now - GRACE &&
          thread.last_inbound_at > @now - STALE_AFTER &&
          reply_owed?(thread)
      }.max_by(&:last_inbound_at)
    end

    # The unanswered message — the thread's latest, since they hold the last word
    # — is one a person answers: not a newsletter, receipt or alert (Skim's
    # per-message broadcast verdict), and addressed to you rather than a thread
    # you were merely copied on. Without this every unanswered promotion reads as
    # "waiting on your reply", and the oldest of them tops the People list.
    def reply_owed?(thread)
      message = thread.latest_message
      return true if message.nil?

      !Contacts::SenderKind.broadcast?(message, provider_hints: false) && !copied_only?(message)
    end

    # You are in Cc and not in To. Positive evidence only: when your address
    # appears nowhere (an alias in To, a list) nothing is demoted.
    def copied_only?(message)
      owner = owner_address_for(message.email_account_id)
      return false if owner.blank?

      message.cc_address.to_s.downcase.include?(owner) && !message.to_address.to_s.downcase.include?(owner)
    end

    def owner_address_for(account_id)
      @owner_addresses ||= readable_accounts.to_h { |account| [ account.id, account.email_address.to_s.downcase ] }
      @owner_addresses[account_id]
    end

    # They owe you and the nudge is due (the AwaitingReply proactive subset),
    # restricted to this counterpart's threads. Longest-silent (oldest outbound) wins.
    def nudge_thread(threads)
      ids = threads.map(&:id).to_set
      due = awaiting_reply.due.select { |thread| ids.include?(thread.id) }
      due.min_by { |thread| thread.last_outbound_at || @now }
    end

    def latest_inbound_message(person)
      return @latest_inbound_by_person[person.id] if primed?(person)

      contact_ids = person.contacts.ids
      return nil if contact_ids.empty?

      EmailMessage.where(contact_id: contact_ids)
                  .accessible_to(@user)
                  .order(received_at: :desc)
                  .first
    end

    # Scout's profile blurb: the person's own summary, else the busiest contact's.
    # Reads loaded contacts in memory (the list path preloads them) so it costs no
    # query; falls back to a query only for an unloaded single record.
    def profile_summary(person)
      own = person.read_attribute(:context_summary).presence
      return own if own

      if person.contacts.loaded?
        person.contacts.select { |c| c.context_summary.present? }
              .max_by { |c| c.email_count.to_i }&.context_summary
      else
        person.contacts.where.not(context_summary: [ nil, "" ])
              .order(email_count: :desc).limit(1).pick(:context_summary)
      end
    end

    def organization_person_standings(organization)
      people =
        if @primed && @org_sample&.key?(organization.id)
          @org_sample[organization.id]
        else
          organization.active_people.includes(:contacts)
                       .select { |person| person.contacts.any?(&:kind_person?) }
                       .sort_by { |person| person.last_email_at || Time.at(0) }
                       .reverse
                       .first(ORG_PEOPLE_SAMPLE)
        end
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

    def readable_accounts
      @readable_accounts ||= @user&.readable_email_accounts&.to_a || []
    end

    # ── Priming (batched loads shared by every counterpart on a list) ──────────

    def primed?(person)
      @primed && @primed_person_ids&.include?(person.id)
    end

    # The org's up-to-ORG_PEOPLE_SAMPLE most-recent person-kind members — the same
    # selection organization_person_standings makes, hoisted so prime can load
    # their thread data in the same batch as the list's people.
    def org_sample_from(people)
      people.select { |person| person.contacts.any?(&:kind_person?) }
            .sort_by { |person| person.last_email_at || Time.at(0) }
            .reverse
            .first(ORG_PEOPLE_SAMPLE)
    end

    def load_org_active_people(organizations)
      return {} if organizations.empty?

      OrganizationMembership.active
                            .where(organization_id: organizations.map(&:id))
                            .includes(person: :contacts)
                            .group_by(&:organization_id)
                            .transform_values { |memberships| memberships.map(&:person) }
    end

    def prime_person_data(people)
      contact_to_person = {}
      people.each do |person|
        @primed_person_ids << person.id
        person.contacts.each { |contact| contact_to_person[contact.id] = person.id }
      end
      contact_ids = contact_to_person.keys
      return if contact_ids.empty?

      prime_threads(contact_ids, contact_to_person)
      prime_latest_inbound(contact_ids, contact_to_person)
    end

    # Mirrors person_threads for the whole batch: the inbox threads (on readable
    # accounts) each person's contacts touch, messages preloaded — grouped by person.
    def prime_threads(contact_ids, contact_to_person)
      return if readable_account_ids.empty?

      scope = EmailMessage.where(contact_id: contact_ids).where.not(email_thread_id: nil)
      scope = scope.where(provider_folder_id: inbox_folder_ids) if inbox_folder_ids.present?
      pairs = scope.distinct.pluck(:contact_id, :email_thread_id)

      thread_ids_by_person = Hash.new { |h, k| h[k] = [] }
      all_thread_ids = []
      pairs.each do |contact_id, thread_id|
        person_id = contact_to_person[contact_id] or next
        thread_ids_by_person[person_id] << thread_id
        all_thread_ids << thread_id
      end

      threads = EmailThread.where(id: all_thread_ids.uniq, email_account_id: readable_account_ids)
                           .includes(:email_messages)
                           .index_by(&:id)
      thread_ids_by_person.each do |person_id, thread_ids|
        @threads_by_person[person_id] = thread_ids.uniq.filter_map { |id| threads[id] }
      end
    end

    # Mirrors latest_inbound_message for the whole batch: the newest accessible
    # message per person (DISTINCT ON per contact, then the newest across the
    # person's contacts), the email thread preloaded for the action-prompt read.
    def prime_latest_inbound(contact_ids, contact_to_person)
      return if readable_account_ids.empty?

      latest_ids = EmailMessage.where(contact_id: contact_ids).accessible_to(@user)
                               .select("DISTINCT ON (contact_id) id")
                               .order("contact_id, received_at DESC, id DESC")
                               .map(&:id)
      return if latest_ids.empty?

      by_person = Hash.new { |h, k| h[k] = [] }
      EmailMessage.where(id: latest_ids).includes(:email_thread).each do |message|
        person_id = contact_to_person[message.contact_id] or next
        by_person[person_id] << message
      end
      by_person.each do |person_id, messages|
        @latest_inbound_by_person[person_id] = messages.max_by { |m| [ m.received_at || Time.at(0), m.id ] }
      end
    end

    def readable_account_ids
      @readable_account_ids ||= @user&.readable_email_accounts&.ids || []
    end

    def inbox_folder_ids
      @inbox_folder_ids ||= Emails::InboxFolders.ids_for(readable_accounts)
    end
  end
end

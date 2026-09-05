# frozen_string_literal: true

module People
  # Scout's assessment of where things stand with a counterpart, derived from
  # the user's active home-feed items (People::Attention). Pure read-over
  # materialized data — no new AI, no extra queries beyond what Attention loaded.
  #
  # Priority:
  #   1. Attention item present (feed items: reply, nudge, decide, pay, chase)
  #      -> needs_you: true, kind: :attention, verb/wait_days/subject from the item.
  #   2. Last exchange with ask or hold signal -> kind: :last_exchange (no needs_you).
  #   3. None -> Result.none.
  #
  # The old "Waiting on your reply for N days." / "No reply to <subject>. Nudge?"
  # sentence templates are gone - verbs and subjects now come from the feed.
  class Standing
    Result = Data.define(:detail, :detail_kind, :money, :needs_you, :thread_id, :overdue_days, :kind,
                         :verb, :subject, :wait_days, :feed_item_id, :email_message_id) do
      def initialize(detail: nil, detail_kind: nil, money: nil, needs_you: false,
                     thread_id: nil, overdue_days: 0, kind: :none,
                     verb: nil, subject: nil, wait_days: 0, feed_item_id: nil, email_message_id: nil)
        super
      end
      def self.none = new(detail: nil)
      # Composed at render time in the current locale.
      def text = People::StandCopy.line(self)
      def present? = detail.present? || detail_kind == :money
    end

    ORG_PEOPLE_SAMPLE = 5

    def self.for_person(person, user:, now: Time.current, attention: nil)
      svc = new(user, now: now)
      attn = attention || People::Attention.new(user, now: now)
      svc.person(person, attn.for(person))
    end

    def self.for_organization(organization, user:, now: Time.current, attention: nil)
      svc = new(user, now: now)
      attn = attention || People::Attention.new(user, now: now)
      svc.organization(organization, attn.for(organization))
    end

    def initialize(user, now: Time.current)
      @user = user
      @now = now
    end

    # Bulk-load everything needed so a whole People list resolves in a handful
    # of queries instead of per-counterpart. Idempotent and additive.
    def prime(people: [], organizations: [])
      @primed = true
      @threads_by_person ||= {}
      @latest_by_thread ||= {}
      @latest_inbound_by_person ||= {}
      @primed_person_ids ||= Set.new
      @org_sample ||= {}

      new_orgs = organizations.reject { |org| @org_sample.key?(org.id) }
      if new_orgs.any?
        active = load_org_active_people(new_orgs)
        new_orgs.each { |org| @org_sample[org.id] = org_sample_from(active[org.id] || []) }
      end

      sampled = @org_sample.values_at(*organizations.map(&:id)).compact.flatten
      to_prime = (people + sampled).reject { |p| @primed_person_ids.include?(p.id) }.uniq(&:id)
      prime_person_data(to_prime) if to_prime.any?
      self
    end

    # Standing for one person. attention_item is People::Attention::Item or nil.
    def person(person, attention_item = nil)
      if attention_item
        result_from_attention(attention_item)
      else
        latest = latest_inbound_message(person)
        thread = latest&.email_thread
        last_subj = latest&.email_thread&.display_subject.to_s.strip.presence ||
                    latest&.subject.to_s.strip.presence

        if thread&.holds_last_word? && thread.last_outbound_at
          result(detail: thread.last_outbound_at.to_date.iso8601, detail_kind: :you_wrote_last,
                 thread: latest&.email_thread, kind: :last_exchange, subject_str: last_subj,
                 email_message_id: latest&.id)
        elsif (ask = People::Ask.for(latest))
          result(detail: ask.text, detail_kind: (ask.kind == :ai ? :ask_ai : :ask_quote),
                 thread: latest&.email_thread, kind: :last_exchange, subject_str: last_subj,
                 email_message_id: latest&.id)
        elsif latest || person.last_email_at.present?
          result(detail: nil, detail_kind: nil, thread: latest&.email_thread,
                 kind: :last_exchange, subject_str: last_subj, email_message_id: latest&.id)
        else
          Result.none
        end
      end
    end

    # Standing for an organization. Only a money item (pay/chase) reaches orgs.
    def organization(_organization, attention_item = nil)
      return Result.none unless attention_item

      result_from_attention(attention_item)
    end

    def threads_for(person) = person_threads(person)
    def latest_inbound_for(person) = latest_inbound_message(person)

    def latest_message_for(thread)
      return @latest_by_thread[thread.id] if @latest_by_thread&.key?(thread.id)

      thread.latest_message
    end

    def sampled_people_for(organization)
      return @org_sample[organization.id] if @primed && @org_sample&.key?(organization.id)

      org_sample_from(organization.active_people.includes(:contacts).to_a)
    end

    private

    def result_from_attention(item)
      fi = item.feed_item
      Result.new(
        detail: item.detail,
        detail_kind: item.detail_kind,
        money: item.money,
        needs_you: true,
        thread_id: item.thread_id,
        overdue_days: item.wait_days,
        kind: :attention,
        verb: item.verb,
        subject: item.subject,
        wait_days: item.wait_days,
        feed_item_id: fi.id,
        email_message_id: item.message&.id
      )
    end

    def result(detail:, detail_kind:, thread: nil, kind: :none, subject_str: nil, email_message_id: nil)
      Result.new(
        detail: detail,
        detail_kind: detail_kind,
        needs_you: false,
        thread_id: thread&.id,
        overdue_days: 0,
        kind: kind,
        subject: subject_str,
        email_message_id: email_message_id
      )
    end

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

    def latest_inbound_message(person)
      return @latest_inbound_by_person[person.id] if primed?(person)

      contact_ids = person.contacts.ids
      return nil if contact_ids.empty?

      EmailMessage.where(contact_id: contact_ids)
                  .accessible_to(@user)
                  .order(received_at: :desc)
                  .first
    end

    def primed?(person)
      @primed && @primed_person_ids&.include?(person.id)
    end

    def org_sample_from(people)
      people.select { |p| p.contacts.any?(&:kind_person?) }
            .sort_by { |p| p.last_email_at || Time.at(0) }
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
                           .index_by(&:id)
      thread_ids_by_person.each do |person_id, thread_ids|
        @threads_by_person[person_id] = thread_ids.uniq.filter_map { |id| threads[id] }
      end

      prime_latest_by_thread(threads.keys)
    end

    def prime_latest_by_thread(thread_ids)
      return if thread_ids.empty?

      cols = %w[
        id email_thread_id contact_id received_at from_address to_address cc_address
        subject category header_list_unsubscribe header_precedence header_auto_submitted
        ai_action_prompt ai_priority ai_ask email_account_id provider_folder_id
      ].join(", ")

      messages = EmailMessage.select("DISTINCT ON (email_thread_id) #{cols}")
                             .where(email_thread_id: thread_ids)
                             .order("email_thread_id, received_at DESC NULLS LAST, id DESC")
      messages.each { |msg| @latest_by_thread[msg.email_thread_id] = msg }
    end

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

    def readable_accounts
      @readable_accounts ||= @user&.readable_email_accounts&.to_a || []
    end

    def readable_account_ids
      @readable_account_ids ||= @user&.readable_email_accounts&.ids || []
    end

    def inbox_folder_ids
      @inbox_folder_ids ||= Emails::InboxFolders.ids_for(readable_accounts)
    end
  end
end

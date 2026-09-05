# frozen_string_literal: true

module People
  # A frozen PORO that bundles every data point needed by the Details rail.
  # Built in one call — People::Profile.for(person, user:) — and cached in the
  # controller. All collections are resolved in a bounded number of queries;
  # the spec gate asserts the budget (≤ 12).
  class Profile
    THREAD_CAP  = 100
    DOCUMENT_CAP = 20
    EVENT_CAP   = 10
    TAG_CAP     = 6
    STALE_DAYS  = 30

    attr_reader :person, :contacts, :primary_contact, :emails,
                :organization, :relationship, :sender_kind,
                :tags, :list_status,
                :read, :patterns, :analyzed_at,
                :counts, :threads,
                :documents, :events, :duplicate_suggestion,
                :attention

    def self.for(person, user:)
      new(person, user: user).tap(&:build!)
    end

    def initialize(person, user:)
      @person = person
      @user   = user
    end

    def sender_kind_taught? = @sender_kind_taught
    def starred?            = @starred
    def analysis_stale?     = @analysis_stale
    def more_threads?       = @more_threads

    def build!
      # 1. Contacts (order: email_count desc).
      @contacts = @person.contacts.includes(:contact_email_aliases, :sender_tags).order(email_count: :desc).to_a
      @primary_contact = @contacts.first

      # 2. Emails list: primary contact email first, then aliases from all contacts.
      raw_emails = []
      @contacts.each do |c|
        raw_emails << [ c.email, c == @primary_contact ]
        c.contact_email_aliases.each { |a| raw_emails << [ a.email, false ] }
      end
      seen = {}
      @emails = raw_emails.each_with_object([]) do |(addr, primary), arr|
        next if seen[addr.downcase]
        seen[addr.downcase] = true
        arr << [ addr, primary ]
      end

      # 3. Org / relationship / sender kind / tags / starred / list status.
      @organization      = @person.primary_organization
      @relationship      = @person.relationship_type.presence ||
                           @primary_contact&.relationship_type
      @sender_kind       = @primary_contact&.sender_kind
      @sender_kind_taught = @primary_contact&.sender_kind_taught? || false
      @tags              = @contacts.flat_map(&:sender_tags).uniq.first(TAG_CAP)
      @starred           = @contacts.any?(&:starred?)
      @list_status       = derive_list_status

      # 4. Scout's read (person fields first, fall back to primary contact).
      @read        = @person.context_summary.presence || @primary_contact&.context_summary
      @patterns    = derive_patterns
      @analyzed_at = @person.analyzed_at || @primary_contact&.analyzed_at
      @analysis_stale = @analyzed_at.nil? || @analyzed_at < STALE_DAYS.days.ago

      # 5. Counts & thread list (message queries, scoped to accessible messages).
      contact_ids = @contacts.map(&:id)
      build_counts_and_threads(contact_ids)
      build_documents(contact_ids)
      build_events(contact_ids)

      # 6. Duplicate suggestion from the primary contact.
      @duplicate_suggestion =
        if (sugg = @primary_contact&.suggested_person)
          { id: sugg.id, name: sugg.display_name, reason: @primary_contact.suggested_reason }
        end

      # 7. The learned attention weight (one query; nil when there is no row) —
      # ::Attention, not People::Attention (the feed reader).
      @attention = ::Attention::Weights.new(@user).for(@person)

      freeze
    end

    private

    def derive_list_status
      statuses = @contacts.map(&:list_status)
      return :blocked  if statuses.include?("blocked")
      return :allowed  if statuses.include?("allowed")
      :neutral
    end

    def derive_patterns
      raw = @person.communication_patterns.presence ||
            @primary_contact&.communication_patterns
      return nil unless raw.is_a?(Hash) && raw.any?

      raw.slice("topics", "tone", "urgency", "primary_role").reject { |_k, v| v.blank? }
    end

    def build_counts_and_threads(contact_ids)
      return set_empty_counts_and_threads if contact_ids.empty?

      # sent? is derived from from_address containing the account email (may be "Name <addr>").
      # Use substring LIKE patterns so "Alice <alice@example.com>" still matches.
      account_emails = @user.readable_email_accounts.loaded? ?
                       @user.readable_email_accounts.map(&:email_address).compact :
                       EmailAccount.joins(:email_account_users)
                                   .where(email_account_users: { user_id: @user.id, can_read: true })
                                   .pluck(:email_address).compact

      base = EmailMessage.where(contact_id: contact_ids).accessible_to(@user)

      # Sent/received tell apart the way EmailMessage#sent? does — lower(from_address)
      # LIKE '%<account address>%' (from_address may be "Name <addr>").
      sent_case, received_case, bind_values = if account_emails.any?
        like_clauses = account_emails.map { "lower(from_address) LIKE ?" }.join(" OR ")
        [ "count(CASE WHEN (#{like_clauses}) THEN 1 END)",
          "count(CASE WHEN NOT (#{like_clauses}) THEN 1 END)",
          account_emails.map { |e| "%#{e.downcase}%" } ]
      else
        [ "0", "count(*)", [] ]
      end

      # Received / first / last come from the person's own messages (contact-linked),
      # minus anything you wrote that happens to carry their contact.
      received_count, first_at, last_at = base.pick(Arel.sql(ActiveRecord::Base.sanitize_sql_array([
        "#{received_case}, min(received_at), max(received_at)", *bind_values
      ])))

      # "Sent" is counted across the person's THREADS, because your own replies
      # carry no contact and would otherwise never be counted. The same thread-wide
      # pass gives each thread's real message count and latest date.
      thread_ids = base.where.not(email_thread_id: nil).distinct.pluck(:email_thread_id)
      stats_sql = ActiveRecord::Base.sanitize_sql_array([
        "email_thread_id, count(*) AS msg_count, max(received_at) AS latest_at, #{sent_case} AS sent_total",
        *bind_values
      ])
      thread_stats = EmailMessage.where(email_thread_id: thread_ids).accessible_to(@user)
                                 .group(:email_thread_id).select(Arel.sql(stats_sql))
                                 .to_a.sort_by { |r| r.latest_at || Time.at(0) }.reverse
      sent_count   = thread_stats.sum { |r| r.sent_total.to_i }
      thread_count = thread_stats.size
      @more_threads = thread_count > THREAD_CAP
      capped_stats  = thread_stats.first(THREAD_CAP)
      capped_ids    = capped_stats.map(&:email_thread_id)

      et_map = EmailThread.where(id: capped_ids).index_by(&:id)
      stat_map = capped_stats.index_by(&:email_thread_id)

      @threads = capped_ids.filter_map do |tid|
        et   = et_map[tid]
        stat = stat_map[tid]
        next unless et && stat

        { id: tid, subject: et.display_subject, count: stat.msg_count.to_i, latest_at: stat.latest_at }
      end

      msg_id_subquery = EmailMessage.where(contact_id: contact_ids).select(:id)
      doc_count_q = Document.joins(:document_email_messages)
                            .where(document_email_messages: { email_message_id: msg_id_subquery })
                            .distinct.count

      @counts = {
        received:         received_count,
        sent:             sent_count,
        threads:          thread_count,
        documents:        doc_count_q,
        first_contact_at: first_at,
        last_contact_at:  last_at
      }
    end

    def set_empty_counts_and_threads
      @threads      = []
      @more_threads = false
      @counts       = { received: 0, sent: 0, threads: 0, documents: 0,
                        first_contact_at: nil, last_contact_at: nil }
    end

    def build_documents(contact_ids)
      return @documents = [] if contact_ids.empty?

      msg_ids = EmailMessage.where(contact_id: contact_ids).select(:id)
      @documents = Document.joins(:document_email_messages)
                           .where(document_email_messages: { email_message_id: msg_ids })
                           .distinct
                           .order(created_at: :desc)
                           .limit(DOCUMENT_CAP)
                           .to_a
    end

    def build_events(contact_ids)
      person_emails = @emails.map { |addr, _primary| addr }
      return @events = [] if person_emails.empty?

      # Attendee conditions: any of the person's emails as {"email": "..."} in the attendees jsonb.
      attendee_conditions = person_emails.map { "attendees @> ?" }.join(" OR ")
      attendee_values     = person_emails.map { |e| [ { email: e } ].to_json }

      msg_id_subquery = contact_ids.any? ? EmailMessage.where(contact_id: contact_ids).select(:id) : nil

      base = CalendarEvent.accessible_to(@user).visible.concrete

      cond = "(#{attendee_conditions})"
      vals = attendee_values.dup
      if msg_id_subquery
        cond += " OR source_email_message_id IN (#{msg_id_subquery.to_sql})"
      end
      base = base.where(cond, *vals)

      # Two queries: upcoming (asc) and past (desc, capped at 5).
      # This ensures upcoming events are always included rather than being
      # eclipsed by a 90-day window that happens to be full of past events.
      now = Time.current
      upcoming = base.where("start_at >= ?", now).order(:start_at).limit(EVENT_CAP).to_a
      past     = base.where("start_at < ?", now).order(start_at: :desc).limit(5).to_a
      @events  = (upcoming + past).first(EVENT_CAP)
    end
  end
end

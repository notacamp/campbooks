# frozen_string_literal: true

module Attention
  # Builds one Attention::Facts per eligible person for a user, in a fixed
  # handful of aggregate queries (never one per person). Pure over the plucked
  # rows after loading; `now` is injectable.
  class Signals
    WINDOW         = 180.days
    FORWARD_WINDOW = 30.days
    REPLY_WINDOW   = 14.days

    BULK_HEADERS   = %w[bulk list junk].freeze
    NOISE_CATEGORIES = %w[notifications promotions social].freeze

    def initialize(user, now: Time.current)
      @user = user
      @now  = now
    end

    # { person_id => Attention::Facts }
    def facts_by_person
      @facts_by_person ||= build_facts
    end

    def person_ids
      @eligible_person_ids || []
    end

    # { org_id => [person_id, ...] }
    def org_memberships
      @org_memberships ||= {}
    end

    # { id => display_name }
    def person_names
      @person_names ||= {}
    end

    private

    def build_facts
      # Step 1: email accounts
      @accounts      = @user.readable_email_accounts.to_a
      @account_ids   = @accounts.map(&:id)
      @owner_addresses = @accounts.map { |a| a.email_address.to_s.downcase }

      # Step 2: contacts
      load_contacts!

      if @eligible_person_ids.empty?
        @org_memberships = {}
        return {}
      end

      # Steps 3-12
      load_people!
      inbound_by_person, outbound_by_thread = load_mail!
      load_events!
      load_feed_verdicts!
      load_learning_decisions!
      load_meetings!
      load_money!
      load_org_memberships!

      assemble_facts(inbound_by_person, outbound_by_thread)
    end

    # Step 2
    def load_contacts!
      rows = Contact
        .where(workspace_id: @user.workspace_id)
        .where.not(person_id: nil)
        .pluck(
          :id, :person_id, :email, :email_count,
          :starred_at, :list_status, :sender_kind, :sender_kind_source,
          :relationship_type, :communication_patterns, :last_email_at
        )

      @contact_by_id       = {}
      @contact_id_by_email = {}
      @contacts_by_person  = Hash.new { |h, k| h[k] = [] }

      rows.each do |id, person_id, email, email_count, starred_at, list_status,
                    sender_kind, sender_kind_source, relationship_type,
                    communication_patterns, last_email_at|
        rec = {
          id: id, person_id: person_id, email: email&.downcase,
          email_count: email_count.to_i, starred_at: starred_at,
          list_status: list_status.to_s, sender_kind: sender_kind.to_s,
          sender_kind_source: sender_kind_source,
          relationship_type: relationship_type,
          communication_patterns: communication_patterns,
          last_email_at: last_email_at
        }
        @contact_by_id[id] = rec
        @contact_id_by_email[email&.downcase] = id if email
        @contacts_by_person[person_id] << rec
      end

      # Eligible: at least one contact with sender_kind "person" and email_count > 0.
      # Rails enum pluck returns the string label, not the integer.
      @eligible_person_ids = @contacts_by_person
        .select { |_pid, contacts| contacts.any? { |c| c[:sender_kind] == "person" && c[:email_count] > 0 } }
        .keys

      @eligible_contact_ids = @contacts_by_person
        .slice(*@eligible_person_ids)
        .values.flatten.map { |c| c[:id] }
    end

    # Step 3
    def load_people!
      rows = Person.where(id: @eligible_person_ids)
        .pluck(:id, :name, :relationship_type, :communication_patterns)

      @person_data = {}
      rows.each do |id, name, relationship_type, communication_patterns|
        @person_data[id] = {
          name: name, relationship_type: relationship_type,
          communication_patterns: communication_patterns
        }
      end

      @person_names = @person_data.to_h do |id, data|
        display_name = data[:name].presence ||
          dominant_contact_for(id)&.dig(:email) || "Unknown"
        [ id, display_name ]
      end
    end

    # Steps 4 & 5: inbound and outbound mail
    def load_mail!
      window_start = @now - WINDOW

      inbound_rows = if @account_ids.any? && @eligible_contact_ids.any?
        EmailMessage
          .where(email_account_id: @account_ids)
          .where("received_at >= ?", window_start)
          .where(contact_id: Contact.where(id: @eligible_contact_ids).select(:id))
          .pluck(
            :id, :contact_id, :email_thread_id, :received_at,
            :from_address, :to_address, :cc_address,
            :viewed_at, :category,
            :header_list_unsubscribe, :header_precedence, :header_auto_submitted,
            :email_account_id
          )
      else
        []
      end

      @inbound_by_msg_id = {}
      inbound_by_person  = Hash.new { |h, k| h[k] = { thread_ids: Set.new, msgs: [] } }

      inbound_rows.each do |id, contact_id, thread_id, received_at, from_address,
                            to_address, _cc_address, viewed_at, category,
                            header_list_unsubscribe, header_precedence, header_auto_submitted,
                            email_account_id|
        next if owner_address?(from_address)

        contact = @contact_by_id[contact_id]
        next unless contact

        account_addr = @accounts.find { |a| a.id == email_account_id }&.email_address.to_s.downcase

        addressed = addressed?(to_address, account_addr) &&
          !bulk?(header_list_unsubscribe, header_precedence, header_auto_submitted, category)

        rec = {
          contact_id: contact_id, thread_id: thread_id,
          received_at: received_at, viewed_at: viewed_at, addressed: addressed,
          account_id: email_account_id
        }
        @inbound_by_msg_id[id] = rec

        person_id = contact[:person_id]
        inbound_by_person[person_id][:thread_ids].add(thread_id) if thread_id
        inbound_by_person[person_id][:msgs] << rec
      end

      # Step 5: outbound mail
      outbound_rows = if @account_ids.any?
        EmailMessage
          .joins(:email_account)
          .where(email_account_id: @account_ids)
          .where("email_messages.received_at >= ?", window_start)
          .where("LOWER(email_messages.from_address) LIKE '%' || LOWER(email_accounts.email_address) || '%'")
          .pluck("email_messages.email_thread_id", "email_messages.received_at")
          .reject { |tid, _| tid.nil? }
      else
        []
      end

      outbound_by_thread = Hash.new { |h, k| h[k] = [] }
      outbound_rows.each { |thread_id, received_at| outbound_by_thread[thread_id] << received_at }
      outbound_by_thread.each_value(&:sort!)

      @reply_data = Hash.new do |h, k|
        h[k] = { replied_count: 0, latencies: [], two_way_thread_ids: Set.new, outbound_thread_ids: Set.new }
      end

      @eligible_person_ids.each do |person_id|
        data = inbound_by_person[person_id]
        data[:msgs].each do |msg|
          thread_id = msg[:thread_id]
          next unless thread_id

          outs = outbound_by_thread[thread_id]
          next if outs.empty?

          reply_time = outs.find { |t| t > msg[:received_at] && t <= msg[:received_at] + REPLY_WINDOW }
          if reply_time
            @reply_data[person_id][:replied_count] += 1
            @reply_data[person_id][:latencies] << (reply_time - msg[:received_at]) / 3600.0
          end
        end

        data[:thread_ids].each do |tid|
          if outbound_by_thread[tid].any?
            @reply_data[person_id][:two_way_thread_ids].add(tid)
            @reply_data[person_id][:outbound_thread_ids].add(tid)
          end
        end
      end

      [ inbound_by_person, outbound_by_thread ]
    end

    # Step 6: events (this user's own actions only)
    def load_events!
      @archived_unread_by_person = Hash.new(0)
      @trashed_by_person         = Hash.new(0)
      @snoozed_by_person         = Hash.new(0)
      @forwarded_by_person       = Hash.new(0)
      @tagged_by_person          = Hash.new(0)

      event_rows = Event
        .where(workspace_id: @user.workspace_id, actor_type: "User", actor_id: @user.id)
        .where(occurred_at: (@now - WINDOW)..)
        .where(name: %w[email.archived email.trashed email.snoozed email.forwarded email.tagged email.bulk_archived])
        .pluck(:name, :subject_type, :subject_id, :payload)

      direct_ids = []
      bulk_ids   = []

      event_rows.each do |name, subject_type, subject_id, payload|
        direct_ids << subject_id if subject_type == "EmailMessage"
        if name == "email.bulk_archived" && payload.is_a?(Hash)
          bulk_ids.concat(Array(payload["ids"]))
        end
      end

      known_ids   = @inbound_by_msg_id.keys.to_set
      missing_ids = ((direct_ids + bulk_ids).uniq - known_ids.to_a)

      @event_extra_map = {}
      if missing_ids.any?
        EmailMessage.where(id: missing_ids)
          .pluck(:id, :contact_id, :viewed_at)
          .each { |id, contact_id, viewed_at| @event_extra_map[id] = { contact_id: contact_id, viewed_at: viewed_at } }
      end

      event_rows.each do |name, subject_type, subject_id, payload|
        if name == "email.bulk_archived"
          ids = payload.is_a?(Hash) ? Array(payload["ids"]) : []
          ids.each do |mid|
            person_id, viewed_at = resolve_contact_person(mid, @event_extra_map)
            next unless person_id
            @archived_unread_by_person[person_id] += 1 if viewed_at.nil?
          end
        elsif subject_type == "EmailMessage"
          person_id, viewed_at = resolve_contact_person(subject_id, @event_extra_map)
          next unless person_id
          case name
          when "email.archived"   then @archived_unread_by_person[person_id] += 1 if viewed_at.nil?
          when "email.trashed"    then @trashed_by_person[person_id] += 1
          when "email.snoozed"    then @snoozed_by_person[person_id] += 1
          when "email.forwarded"  then @forwarded_by_person[person_id] += 1
          when "email.tagged"     then @tagged_by_person[person_id] += 1
          end
        end
      end
    end

    # Step 7: feed verdicts
    def load_feed_verdicts!
      @feed_acted_by_person     = Hash.new(0)
      @feed_dismissed_by_person = Hash.new(0)

      verdict_rows = @user.feed_items
        .where(subject_type: "EmailMessage")
        .where("acted_at >= :t OR dismissed_at >= :t", t: @now - WINDOW)
        .pluck(:subject_id, :acted_at, :dismissed_at)

      known_ids    = @inbound_by_msg_id.keys.to_set
      missing_ids  = verdict_rows.map(&:first).uniq - known_ids.to_a

      # Combine the feed extra-map with the event extra-map (both resolve the same way)
      feed_extra = {}
      if missing_ids.any?
        EmailMessage.where(id: missing_ids)
          .pluck(:id, :contact_id, :viewed_at)
          .each { |id, cid, va| feed_extra[id] = { contact_id: cid, viewed_at: va } }
      end

      verdict_rows.each do |msg_id, acted_at, dismissed_at|
        person_id, = resolve_contact_person(msg_id, feed_extra)
        next unless person_id

        @feed_acted_by_person[person_id]    += 1 if acted_at
        @feed_dismissed_by_person[person_id] += 1 if dismissed_at
      end
    end

    # Step 8: learning decisions
    def load_learning_decisions!
      @skim_archive_by_person = Hash.new(0)
      @skim_keep_by_person    = Hash.new(0)
      @taught_by_person       = {}

      ld_rows = LearningDecision
        .where(user_id: @user.id, domain: %w[email_skim attention])
        .where.not(contact_id: nil)
        .where(created_at: (@now - WINDOW)..)
        .pluck(:domain, :contact_id, :label, :created_at)

      ld_rows.each do |domain, contact_id, label, created_at|
        contact = @contact_by_id[contact_id]
        next unless contact
        person_id = contact[:person_id]

        case domain
        when "email_skim"
          case label
          when "archive"          then @skim_archive_by_person[person_id] += 1
          when "keep", "promote"  then @skim_keep_by_person[person_id] += 1
          end
        when "attention"
          next unless %w[important unimportant].include?(label)
          existing = @taught_by_person[person_id]
          @taught_by_person[person_id] = { label: label, created_at: created_at } if existing.nil? || existing[:created_at] < created_at
        end
      end
    end

    # Step 9: meetings
    def load_meetings!
      @meetings_by_person = Hash.new(0)

      event_rows = CalendarEvent
        .accessible_to(@user)
        .visible
        .where(start_at: (@now - WINDOW)..(@now + FORWARD_WINDOW))
        .where("jsonb_array_length(attendees) > 0")
        .pluck(:attendees)

      event_rows.each do |attendees_json|
        person_ids_in_event = Set.new
        Array(attendees_json).each do |a|
          email = (a.is_a?(Hash) ? a["email"] : a.to_s)&.downcase
          next if email.blank? || @owner_addresses.include?(email)
          contact_id = @contact_id_by_email[email]
          next unless contact_id
          contact = @contact_by_id[contact_id]
          next unless contact && @eligible_person_ids.include?(contact[:person_id])
          person_ids_in_event.add(contact[:person_id])
        end
        person_ids_in_event.each { |pid| @meetings_by_person[pid] += 1 }
      end
    end

    # Step 10: money (settled_at tracked; due_date/delay not available in current schema)
    def load_money!
      @invoices_by_person      = Hash.new { |h, k| h[k] = Set.new }
      @settled_by_person       = Hash.new { |h, k| h[k] = Set.new }
      @settle_delays_by_person = Hash.new { |h, k| h[k] = [] }

      # Use a subquery to compose Document.accessible_to without join-shape conflicts.
      money_rows = DocumentEmailMessage
        .joins(:document, :email_message)
        .where(documents: { id: Document.accessible_to(@user).select(:id) })
        .where(documents: { workspace_id: @user.workspace_id, document_type: Document::MONEY_TYPES })
        .pluck("email_messages.contact_id", "documents.id", "documents.settled_at")

      money_rows.each do |contact_id, doc_id, settled_at|
        contact = @contact_by_id[contact_id]
        next unless contact
        person_id = contact[:person_id]
        next unless @eligible_person_ids.include?(person_id)

        @invoices_by_person[person_id].add(doc_id)
        @settled_by_person[person_id].add(doc_id) if settled_at
        # Schema has no due_date column; median_settle_delay_days stays nil
      end
    end

    # Step 12: org memberships
    def load_org_memberships!
      rows = OrganizationMembership.active.where(person_id: @eligible_person_ids)
        .pluck(:organization_id, :person_id)

      @org_memberships = Hash.new { |h, k| h[k] = [] }
      rows.each { |org_id, person_id| @org_memberships[org_id] << person_id }
      @org_memberships = @org_memberships.to_h
    end

    # Step 11 + assemble
    def assemble_facts(inbound_by_person, _outbound_by_thread)
      @eligible_person_ids.to_h do |person_id|
        contacts     = @contacts_by_person[person_id]
        dominant     = dominant_contact_for(person_id)
        inbound_msgs = inbound_by_person[person_id][:msgs]
        reply        = @reply_data[person_id]

        starred = contacts.any? { |c| c[:starred_at].present? }
        allowed = contacts.any? { |c| c[:list_status] == "allowed" }
        blocked = contacts.all? { |c| c[:list_status] == "blocked" }

        sender_kind = case dominant&.dig(:sender_kind)
        when "person"  then "person"
        when "service" then "service"
        end

        person_rel  = @person_data[person_id]&.dig(:relationship_type)
        rel_type    = person_rel.presence || dominant&.dig(:relationship_type)

        person_pats = @person_data[person_id]&.dig(:communication_patterns)
        urgency     = extract_urgency(person_pats) || extract_urgency(dominant&.dig(:communication_patterns))

        last_email_at = contacts.filter_map { |c| c[:last_email_at] }.max
        last_inbound  = inbound_msgs.filter_map { |m| m[:received_at] }.max
        last_activity = [ last_email_at, last_inbound ].compact.max

        inbound_count   = inbound_msgs.size
        addressed_count = inbound_msgs.count { |m| m[:addressed] }
        opened_count    = inbound_msgs.count { |m| m[:viewed_at].present? }

        latencies        = reply[:latencies]
        median_reply_h   = latencies.any? ? median(latencies) : nil
        two_way_threads  = reply[:two_way_thread_ids].size
        outbound_threads = reply[:outbound_thread_ids].size

        delays             = @settle_delays_by_person[person_id]
        median_settle_days = delays.any? ? median(delays) : nil

        taught_data = @taught_by_person[person_id]
        taught      = taught_data&.dig(:label)

        facts = Facts.new(
          inbound_count:            inbound_count,
          addressed_count:          addressed_count,
          replied_count:            reply[:replied_count],
          median_reply_hours:       median_reply_h,
          two_way_threads:          two_way_threads,
          outbound_threads:         outbound_threads,
          opened_count:             opened_count,
          meetings_count:           @meetings_by_person[person_id],
          invoices_count:           @invoices_by_person[person_id].size,
          settled_count:            @settled_by_person[person_id].size,
          median_settle_delay_days: median_settle_days,
          archived_unread_count:    @archived_unread_by_person[person_id],
          trashed_count:            @trashed_by_person[person_id],
          snoozed_count:            @snoozed_by_person[person_id],
          forwarded_count:          @forwarded_by_person[person_id],
          tagged_count:             @tagged_by_person[person_id],
          feed_acted_count:         @feed_acted_by_person[person_id],
          feed_dismissed_count:     @feed_dismissed_by_person[person_id],
          skim_archive_count:       @skim_archive_by_person[person_id],
          skim_keep_count:          @skim_keep_by_person[person_id],
          starred:                  starred,
          allowed:                  allowed,
          blocked:                  blocked,
          sender_kind:              sender_kind,
          relationship_type:        rel_type,
          urgency_level:            urgency,
          taught:                   taught,
          last_activity_at:         last_activity
        )

        [ person_id, facts ]
      end
    end

    def resolve_contact_person(msg_id, extra_map)
      rec = @inbound_by_msg_id[msg_id] || extra_map[msg_id]
      return nil unless rec
      contact = @contact_by_id[rec[:contact_id]]
      return nil unless contact
      [ contact[:person_id], rec[:viewed_at] ]
    end

    def dominant_contact_for(person_id)
      @contacts_by_person[person_id].max_by { |c| c[:email_count] }
    end

    def owner_address?(address)
      addr = address.to_s.downcase
      @owner_addresses.any? { |o| addr.include?(o) }
    end

    def addressed?(to_address, account_addr)
      return false if account_addr.blank?
      to_address.to_s.downcase.include?(account_addr)
    end

    def bulk?(header_list_unsubscribe, header_precedence, header_auto_submitted, category)
      return true if header_list_unsubscribe.to_s.strip.present?
      return true if BULK_HEADERS.include?(header_precedence.to_s.strip.downcase)
      auto = header_auto_submitted.to_s.strip.downcase
      return true if auto.present? && auto != "no"
      NOISE_CATEGORIES.include?(category.to_s)
    end

    def extract_urgency(patterns)
      return nil unless patterns.is_a?(Hash)
      patterns["urgency_level"].presence
    end

    def median(arr)
      return nil if arr.empty?
      sorted = arr.sort
      mid    = sorted.size / 2
      sorted.size.odd? ? sorted[mid].to_f : (sorted[mid - 1] + sorted[mid]).to_f / 2
    end
  end
end

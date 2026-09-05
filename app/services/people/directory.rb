# frozen_string_literal: true

module People
  # Builds the full People::Counterpart array for a user — persons and
  # organizations, with Scout standings and priority scores — using the same
  # logic that previously lived inline in PeopleLayout. Extracted so the
  # background People::StandingsRefreshJob can run the computation without an
  # HTTP request.
  #
  # Usage: `People::Directory.new(user, workspace:, now: Time.current).counterparts`
  #
  # The result is the unfiltered, unpaginated list of all counterparts. No
  # partitioning into Need-you / Recent happens here; the caller (People::Standings
  # for the refresh path, PeopleLayout for the request path) handles that.
  class Directory
    def initialize(user, workspace:, now: Time.current)
      @user = user
      @workspace = workspace
      @now = now
    end

    # Returns every eligible People::Counterpart (persons + organizations) for
    # this user, with standings and scores computed. Idempotent - repeated calls
    # return the same set.
    def counterparts
      attention = People::Attention.new(@user, now: @now)
      persons   = eligible_persons
      orgs      = eligible_orgs

      prefetch_org_counts(orgs)
      prime_standing(people: persons, organizations: orgs)

      person_rows = persons.select { |person| listable_person?(person) }
                           .map { |person| person_counterpart(person, attention) }

      # Add data flags: new sender, unread message, attachment.
      add_person_flags!(person_rows, attention)

      # Fold group threads: Need-you persons sharing a thread_id become one row.
      person_rows = fold_group_threads(person_rows, attention)

      # Orgs: a row ONLY when there is a money attention item for this org.
      # No Recent org rows.
      person_rows_by_id = person_rows.index_by(&:id)
      org_rows = orgs.filter_map do |org|
        org_item = attention.for(org)
        next unless org_item

        org_counterpart(org, person_rows_by_id, org_item)
      end

      person_rows + org_rows
    end

    # Build a single counterpart for one person, or nil when the person is no
    # longer eligible (email_count dropped to zero, blocked, etc.).
    def counterpart_for(person)
      attention = People::Attention.new(@user, now: @now)
      people_standing.prime(people: [ person ])
      # Re-check the same eligibility criteria used in eligible_persons so that
      # refresh_counterpart! can delete a row when the person becomes ineligible.
      contacts = person.contacts.to_a
      return nil if contacts.none? { |c| c.kind_person? && c.email_count.to_i > 0 }
      return nil unless listable_person?(person)

      # The same data flags the full directory computes (new / unread / snippet /
      # can_reply …), so a single-row refresh never strips the row of its chips.
      rows = [ person_counterpart(person, attention) ]
      add_person_flags!(rows, attention)
      rows.first
    end

    # ── Exposed for People::Standings.refresh! ────────────────────────────────
    def readable_account_ids = @readable_account_ids ||= readable_accounts.map(&:id)
    def inbox_folder_ids     = @inbox_folder_ids     ||= Emails::InboxFolders.ids_for(readable_accounts)

    # Org metadata for the `data` JSONB column.
    def org_row_data(org_id, extra: {})
      {
        "people_count"   => @org_people_counts&.dig(org_id) || 0,
        "services_count" => @org_services_counts&.dig(org_id) || 0
      }.merge(extra)
    end

    private

    # ── Eligibility ──────────────────────────────────────────────────────────

    def eligible_persons
      person_ids = Contact.where(workspace_id: @workspace.id, sender_kind: Contact.sender_kinds[:person])
                          .where("email_count > 0")
                          .where.not(person_id: nil)
                          .select(:person_id)
      @workspace.people.where(id: person_ids).includes(:contacts, :primary_organization).to_a
    end

    def eligible_orgs
      org_ids = OrganizationMembership.joins(person: :contacts)
                                      .where(contacts: { workspace_id: @workspace.id })
                                      .where("contacts.email_count > 0")
                                      .select(:organization_id)
      @workspace.organizations.where(id: org_ids).to_a
    end

    def listable_person?(person)
      contacts = person.contacts.to_a
      return false if owner_person?(person, contacts)
      return false if contacts.all?(&:blocked?)

      judged = contacts.select { |contact| contact.kind_person? && contact.email_count.to_i.positive? }
      return true if judged.any? { |contact| contact.sender_kind_source.present? }

      sample = people_standing.threads_for(person).filter_map { |t| people_standing.latest_message_for(t) }
      sample = [ people_standing.latest_inbound_for(person) ].compact if sample.empty?
      !Contacts::SenderKind.service?(sample, provider_hints: false)
    end

    def owner_person?(person, contacts)
      return true if person.relationship_type == "self"

      owner = readable_accounts.map { |account| account.email_address.to_s.downcase }
      contacts.any? { |contact| owner.include?(contact.email.to_s.downcase) }
    end

    # ── Counterpart construction ──────────────────────────────────────────────

    def person_facts(person, standing, item_score: 0.0)
      People::Priority.facts_for(
        standing: standing,
        threads: people_standing.threads_for(person),
        contacts: person.contacts.to_a,
        relationship_type: person.relationship_type,
        last_activity: person.last_email_at,
        item_score: item_score
      )
    end

    def org_lead(org, person_rows_by_id)
      people_standing.sampled_people_for(org)
                     .map { |member| person_rows_by_id[member.id] }
                     .compact
                     .max_by(&:priority)
    end

    def person_counterpart(person, attention)
      attention_item = attention.for(person)
      standing = people_standing.person(person, attention_item)
      item_score = attention_item&.feed_item&.score.to_f
      facts = person_facts(person, standing, item_score: item_score)

      People::Counterpart.new(
        kind: :person,
        record: person,
        name: person.display_name,
        subtitle: person.organization_name.presence,
        avatar_email: person_primary_email(person).presence || person.display_name,
        avatar_initial: nil,
        last_activity: person.last_email_at,
        standing: standing,
        facts: facts,
        score: People::Priority.score(facts, now: @now),
        data: {}
      )
    end

    def person_primary_email(person)
      return person.primary_email unless person.contacts.loaded?

      person.contacts.max_by { |contact| contact.email_count.to_i }&.email
    end

    def org_counterpart(org, person_rows_by_id = {}, org_attention_item = nil)
      people_count   = @org_people_counts&.dig(org.id) || 0
      services_count = @org_services_counts&.dig(org.id) || 0
      last_activity  = @org_last_activity&.dig(org.id)
      standing       = people_standing.organization(org, org_attention_item)
      lead           = org_lead(org, person_rows_by_id)
      item_score     = org_attention_item&.feed_item&.score.to_f

      facts = if lead&.facts
                lead.facts.with(standing: standing, item_score: item_score)
      else
                People::Priority.facts_for(standing: standing, threads: [], contacts: [],
                                           relationship_type: nil, last_activity: last_activity,
                                           item_score: item_score)
      end

      People::Counterpart.new(
        kind: :organization,
        record: org,
        name: org.name,
        subtitle: org_subtitle(people_count, services_count),
        avatar_email: nil,
        avatar_initial: org.name.to_s[0].to_s.upcase.presence || "?",
        last_activity: last_activity,
        standing: standing,
        facts: facts,
        score: People::Priority.score(facts, now: @now),
        data: { "people_count" => people_count, "services_count" => services_count }
      )
    end

    def org_subtitle(people_count, services_count)
      parts = [ I18n.t("people.index.organization") ]
      parts << I18n.t("people.index.people_count", count: people_count) if people_count.positive?
      parts << I18n.t("people.index.services_count", count: services_count) if services_count.positive?
      parts.join(" · ")
    end

    # ── Person data flags (new / unread / has_attachment / starred / can_reply / can_done / tags) ──

    DONE_KINDS = %w[reply_reminder reply_owed email_action follow_up].freeze

    # How many chips a list row shows. Both tag sources (sender + email) feed one
    # capped cluster so a dense row never overflows on mobile.
    TAG_CAP = 2

    def add_person_flags!(rows, attention)
      return if rows.empty?

      # One grouped query: threads with at least one unread inbound message.
      standing_thread_ids = rows.filter_map { |cp| cp.standing.thread_id }
      unread_thread_set   = unread_thread_ids(standing_thread_ids)

      # Batch sendable account check: one EmailAccountUser lookup per account.
      sendable_account_ids = sendable_account_id_set

      # Batch email_account_id lookup for standing messages.
      standing_msg_ids = rows.filter_map { |cp| cp.standing.email_message_id }
      msg_account_map  = standing_msg_ids.any? ?
        EmailMessage.where(id: standing_msg_ids).pluck(:id, :email_account_id).to_h : {}

      # Two batched tag loads for the whole list (one query each): the person's
      # own sender tags (per contact) and the email's tags (thread-union), both
      # visible-only and workspace-scoped like every other chip render site.
      all_contact_ids       = rows.flat_map { |cp| cp.record&.contacts&.to_a }.compact.map(&:id)
      sender_tags_by_contact = sender_tags_for(all_contact_ids)
      inbox_tags_by_thread   = inbox_tags_for(standing_thread_ids)

      rows.map! do |cp|
        person = cp.record
        next cp unless person

        contacts = person.contacts.to_a
        item = attention.for(person)

        # can_reply: standing message exists and account is sendable.
        msg_id         = cp.standing.email_message_id
        acct_id        = msg_account_map[msg_id]

        latest_inbound = people_standing.latest_inbound_for(person)

        data = cp.data.dup
        data["new"]            = new_sender?(contacts, people_standing.threads_for(person))
        # Unread when the standing thread has unread inbound mail, or — the inbox
        # reading — when the newest thing they sent has not been read yet.
        data["unread"]         = unread_thread_set.include?(cp.standing.thread_id) ||
                                 (latest_inbound.present? && !latest_inbound.read)
        data["has_attachment"] = item&.message&.has_attachment? || false
        data["starred"]        = contacts.any?(&:starred?)
        data["contact_id"]     = contacts.max_by { |c| c.email_count.to_i }&.id
        data["snippet"]        = snippet_for(latest_inbound)
        data["can_reply"]      = msg_id.present? && acct_id && sendable_account_ids.include?(acct_id)
        data["can_done"]       = item.present? && DONE_KINDS.include?(item.feed_item.kind)
        data["tags"]           = row_tags(contacts, cp.standing.thread_id,
                                          sender_tags_by_contact, inbox_tags_by_thread)

        cp.with(data: data)
      end
    end

    # ── Row tag chips (sender + email, capped) ────────────────────────────────
    # Materialized into data["tags"] so the row renders from the table with no
    # extra query. Same staleness as the row's subject/snippet: the cluster
    # refreshes with the standings, not on every manual tag edit.

    # Sender (contact-level) tags for a set of contacts — visible, workspace
    # scoped — keyed by contact_id. One query for the whole list.
    def sender_tags_for(contact_ids)
      return {} if contact_ids.blank?

      map = {}
      ContactTag.joins(:tag)
                .where(contact_id: contact_ids, tags: { workspace_id: @workspace.id, hidden: false })
                .order("tags.name")
                .pluck(:contact_id, "tags.id", "tags.name", "tags.color")
                .each { |cid, id, name, color| (map[cid] ||= []) << tag_hash(id, name, color) }
      map
    end

    # Email (message-level) tags for a set of threads — thread-union, visible,
    # workspace scoped — keyed by thread_id. One query for the whole list.
    def inbox_tags_for(thread_ids)
      return {} if thread_ids.blank?

      map = {}
      EmailMessageTag.joins(:email_message, :tag)
                     .where(email_messages: { email_thread_id: thread_ids })
                     .where(tags: { workspace_id: @workspace.id, hidden: false })
                     .distinct
                     .order("tags.name")
                     .pluck("email_messages.email_thread_id", "tags.id", "tags.name", "tags.color")
                     .each { |tid, id, name, color| (map[tid] ||= []) << tag_hash(id, name, color) }
      map
    end

    # Sender tags first (they identify the person), then the email's own tags,
    # de-duplicated by id and capped. Deterministic order (both loads sort by
    # name) keeps the materialized JSONB stable so it doesn't churn broadcasts.
    def row_tags(contacts, thread_id, sender_map, inbox_map)
      sender = contacts.flat_map { |c| sender_map[c.id] || [] }
      inbox  = (thread_id && inbox_map[thread_id]) || []
      (sender + inbox).uniq { |t| t["id"] }.first(TAG_CAP)
    end

    def tag_hash(id, name, color)
      { "id" => id, "name" => name, "color" => color }
    end

    # One line of the newest message from them, for rows without a Scout read:
    # the provider snippet when present, else the body as plain text.
    def snippet_for(message)
      return nil unless message

      text = message.summary.presence || html_to_text(message.body)
      text&.squish&.truncate(120).presence
    end

    def html_to_text(html)
      return nil if html.blank?

      fragment = Loofah.fragment(html.to_s)
      fragment.xpath(".//style|.//script|.//head|.//title").each(&:remove)
      fragment.to_text(encode_special_chars: false)
    rescue StandardError
      ActionController::Base.helpers.strip_tags(html.to_s)
    end

    # True for a person with at most 1 message and no outbound from you.
    def new_sender?(contacts, threads)
      email_count = contacts.sum { |c| c.email_count.to_i }
      return false unless email_count <= 1

      threads.none? { |t| t.last_outbound_at.present? }
    end

    # One query: thread ids that have at least one inbound message with read: false.
    def unread_thread_ids(thread_ids)
      return Set.new if thread_ids.empty?

      set = EmailMessage.where(email_thread_id: thread_ids)
                        .where(read: false)
                        .where.not(from_address: readable_accounts.map(&:email_address).map(&:downcase))
                        .distinct
                        .pluck(:email_thread_id)
                        .to_set
      set
    rescue StandardError
      Set.new
    end

    # ── Group-thread folding ─────────────────────────────────────────────────
    # Among Need-you person rows sharing a thread_id, keep the highest-scoring
    # one, set its name to all participants (max 3 + "+N"), data["participant_ids"],
    # and drop the others.

    def fold_group_threads(rows, _attention)
      need_you_rows, recent_rows = rows.partition(&:needs_you?)
      dropped = Set.new

      folded = need_you_rows.sort_by { |cp| -cp.priority }.filter_map do |cp|
        next nil if dropped.include?(cp.id)

        tid = cp.standing.thread_id
        next cp if tid.blank?

        members = rows.select do |other|
          other.id != cp.id && !dropped.include?(other.id) && other.record && on_thread?(other.record, tid)
        end
        next cp if members.empty?

        members.each { |member| dropped << member.id }
        names = ([ cp.name ] + members.map(&:name)).uniq
        name = names.size <= 3 ? names.join(", ") : "#{names.first(3).join(', ')} +#{names.size - 3}"
        cp.with(name: name, data: cp.data.merge("participant_ids" => [ cp.id ] + members.map(&:id)))
      end

      folded + recent_rows.reject { |cp| dropped.include?(cp.id) }
    end

    # Does this person's inbox mail include the thread? (primed, no query)
    def on_thread?(person, thread_id)
      people_standing.threads_for(person).any? { |thread| thread.id == thread_id }
    end

    # ── Batch org count queries ───────────────────────────────────────────────

    def prefetch_org_counts(orgs)
      return if orgs.empty?

      org_ids = orgs.map(&:id)
      @org_people_counts   = batch_person_counts(org_ids)
      @org_services_counts = batch_service_counts(org_ids)
      @org_last_activity   = batch_last_activity(org_ids)
    end

    def batch_person_counts(org_ids)
      result = OrganizationMembership.active
                                     .where(organization_id: org_ids)
                                     .joins(person: :contacts)
                                     .where(contacts: { sender_kind: Contact.sender_kinds[:person] })
                                     .group(:organization_id)
                                     .count("DISTINCT people.id")
      result.transform_keys(&:itself)
    end

    def batch_service_counts(org_ids)
      result = Contact.joins(person: :organization_memberships)
                      .where(organization_memberships: { organization_id: org_ids })
                      .where(sender_kind: Contact.sender_kinds[:service])
                      .group("organization_memberships.organization_id")
                      .count("DISTINCT contacts.id")
      result.transform_keys(&:itself)
    end

    # MAX(received_at) per org through contacts → people → memberships, as one
    # grouped ActiveRecord query (no raw SQL).
    def batch_last_activity(org_ids)
      EmailMessage.joins(contact: { person: :organization_memberships })
                  .where(organization_memberships: { organization_id: org_ids })
                  .group("organization_memberships.organization_id")
                  .maximum(:received_at)
    end

    # ── Standing ─────────────────────────────────────────────────────────────

    def people_standing
      @people_standing ||= People::Standing.new(@user, now: @now)
    end

    def prime_standing(people: [], organizations: [])
      people_standing.prime(people: people, organizations: organizations)
    end

    # ── Sendable account set (batched, cached) ────────────────────────────────
    # Returns the set of email_account_ids the current user can SEND from —
    # used to compute data["can_reply"] for each row in one bulk query.

    def sendable_account_id_set
      @sendable_account_id_set ||=
        EmailAccountUser.where(user_id: @user.id, can_send: true, email_account_id: readable_account_ids)
                        .pluck(:email_account_id)
                        .to_set
    end

    # ── Shared inbox scoping ──────────────────────────────────────────────────

    def readable_accounts
      @readable_accounts ||= @user.readable_email_accounts.ordered.to_a
    end
  end
end

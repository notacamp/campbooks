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
    # this user, with standings and scores computed. Idempotent — repeated calls
    # return the same set.
    def counterparts
      persons = eligible_persons
      orgs    = eligible_orgs

      prefetch_org_counts(orgs)
      prime_standing(people: persons, organizations: orgs)

      person_rows = persons.select { |person| listable_person?(person) }
                           .map { |person| person_counterpart(person) }
      person_rows_by_id = person_rows.index_by(&:id)
      org_rows = orgs.map { |org| org_counterpart(org, person_rows_by_id) }

      person_rows + org_rows
    end

    # Build a single counterpart for one person — used as a fallback when the
    # standings table has no row for a person (e.g. right after deploy). Primes
    # the shared standing instance for that person and builds the full counterpart.
    def counterpart_for(person)
      people_standing.prime(people: [ person ])
      person_counterpart(person)
    end

    # ── Exposed for People::Standings.refresh! ────────────────────────────────
    def readable_account_ids = @readable_account_ids ||= readable_accounts.map(&:id)
    def inbox_folder_ids     = @inbox_folder_ids     ||= Emails::InboxFolders.ids_for(readable_accounts)

    # Org metadata for the `data` JSONB column (people_count + services_count),
    # computed in batch by #prefetch_org_counts.
    def org_row_data(org_id)
      {
        "people_count"   => @org_people_counts&.dig(org_id) || 0,
        "services_count" => @org_services_counts&.dig(org_id) || 0
      }
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

    # Defense in depth over the eligibility query (mirrors PeopleLayout#listable_person?).
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

    def person_facts(person, standing)
      People::Priority.facts_for(
        standing: standing,
        threads: people_standing.threads_for(person),
        contacts: person.contacts.to_a,
        latest_inbound: people_standing.latest_inbound_for(person),
        relationship_type: person.relationship_type,
        last_activity: person.last_email_at,
        now: @now
      )
    end

    def org_lead(org, person_rows_by_id)
      people_standing.sampled_people_for(org)
                     .map { |member| person_rows_by_id[member.id] || person_counterpart(member) }
                     .max_by(&:priority)
    end

    def standing_only_score(standing, last_activity)
      facts = People::Priority.facts_for(standing: standing, threads: [], contacts: [], latest_inbound: nil,
                                         relationship_type: nil, last_activity: last_activity, now: @now)
      People::Priority.score(facts, now: @now)
    end

    def person_counterpart(person)
      standing = people_standing.person(person)
      facts = person_facts(person, standing)

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
        score: People::Priority.score(facts, now: @now)
      )
    end

    def person_primary_email(person)
      return person.primary_email unless person.contacts.loaded?

      person.contacts.max_by { |contact| contact.email_count.to_i }&.email
    end

    def org_counterpart(org, person_rows_by_id = {})
      people_count   = @org_people_counts&.dig(org.id) || 0
      services_count = @org_services_counts&.dig(org.id) || 0
      last_activity  = @org_last_activity&.dig(org.id)
      standing       = people_standing.organization(org)
      lead           = org_lead(org, person_rows_by_id)

      People::Counterpart.new(
        kind: :organization,
        record: org,
        name: org.name,
        subtitle: org_subtitle(people_count, services_count),
        avatar_email: nil,
        avatar_initial: org.name.to_s[0].to_s.upcase.presence || "?",
        last_activity: last_activity,
        standing: standing,
        facts: lead&.facts,
        score: lead&.score || standing_only_score(standing, last_activity)
      )
    end

    def org_subtitle(people_count, services_count)
      parts = [ I18n.t("people.index.organization") ]
      parts << I18n.t("people.index.people_count", count: people_count) if people_count.positive?
      parts << I18n.t("people.index.services_count", count: services_count) if services_count.positive?
      parts.join(" · ")
    end

    # ── Batch org count queries ───────────────────────────────────────────────
    # Called once before building org counterparts. Three queries for all orgs
    # instead of three per org.

    def prefetch_org_counts(orgs)
      return if orgs.empty?

      org_ids = orgs.map(&:id)
      @org_people_counts   = batch_person_counts(org_ids)
      @org_services_counts = batch_service_counts(org_ids)
      @org_last_activity   = batch_last_activity(org_ids)
    end

    # COUNT of distinct active people with a person-kind contact per org.
    def batch_person_counts(org_ids)
      result = OrganizationMembership.active
                                     .where(organization_id: org_ids)
                                     .joins(person: :contacts)
                                     .where(contacts: { sender_kind: Contact.sender_kinds[:person] })
                                     .group(:organization_id)
                                     .count("DISTINCT people.id")
      result.transform_keys(&:itself)
    end

    # COUNT of service-kind contacts per org (via org -> people -> contacts).
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

    # ── Shared inbox scoping ──────────────────────────────────────────────────

    def readable_accounts
      @readable_accounts ||= @user.readable_email_accounts.ordered.to_a
    end
  end
end

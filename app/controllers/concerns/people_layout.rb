# frozen_string_literal: true

require "pagy/extras/array"

# Shared chrome for the People place: the bold-layout gate, the "email" three-pane
# layout, the readable-mailbox scoping the inbox uses, the streams count for the
# segmented control, and the counterpart list (persons + organizations, split into
# Need-you / Recent by People::Standing). Included by PeopleController and
# People::OrganizationsController (which share the person-list left pane) and, for
# the gate + streams count, People::StreamsController.
module PeopleLayout
  extend ActiveSupport::Concern
  include Pagy::Backend

  PEOPLE_PER_PAGE = 30

  included do
    before_action :require_bold_layout_enabled
    layout "email"
    helper_method :people_active_tab, :streams_count
  end

  # Which segment tab is lit ("People" or "Streams"). Overridden by the streams
  # controllers.
  def people_active_tab = :people

  private

  # ── Left pane: the counterpart list ───────────────────────────────────────
  # Persons (with a person-kind contact that has mail) + organizations (with a
  # member contact that has mail), each carrying its Scout standing and a
  # People::Priority score. Need-you first — genuine asks, weighted by how much
  # the relationship matters and how long they've waited — then Recent, recency-
  # led and relationship-tiebroken (paginated + infinite scroll).
  def build_people_list
    @query = params[:q].to_s.strip
    lazy_backfill_sender_kinds

    persons = eligible_persons
    orgs    = eligible_orgs
    prime_standing(people: persons, organizations: orgs)

    person_rows = persons.select { |person| listable_person?(person) }
                         .map { |person| person_counterpart(person) }
    facts_by_person = person_rows.to_h { |row| [ row.id, row.facts ] }
    org_rows = orgs.map { |org| org_counterpart(org, facts_by_person) }

    need_you, recent = (person_rows + org_rows).partition(&:needs_you?)
    @need_you = need_you.sort_by { |c| [ -c.priority, -activity_epoch(c) ] }
    recent_sorted = recent.sort_by { |c| [ -c.priority, -activity_epoch(c) ] }
    @recent_pagy, @recent = pagy_array(recent_sorted, limit: PEOPLE_PER_PAGE)
  end

  def activity_epoch(counterpart) = counterpart.last_activity&.to_i || 0

  def list_now = (@now ||= Time.current)

  # Defense in depth over the eligibility query. `sender_kind` is :person by
  # column default until Contacts::SenderKindBackfillJob (debounced, async) has
  # judged the contact — so until then a newsletter passes as a person. Judge the
  # never-classified ones here, in memory, by the backfill's own majority rule
  # over the newest message of each loaded thread (a thread you replied in ends
  # on your message, which reads as a person — rightly). Also keep out the
  # mailbox owner, whose own contact Contacts::Identifier creates from outbound
  # mail, and senders you blocked.
  def listable_person?(person)
    contacts = person.contacts.to_a
    return false if owner_person?(person, contacts)
    return false if contacts.all?(&:blocked?)

    judged = contacts.select { |contact| contact.kind_person? && contact.email_count.to_i.positive? }
    return true if judged.any? { |contact| contact.sender_kind_source.present? }

    sample = people_standing.threads_for(person).filter_map(&:latest_message)
    sample = [ people_standing.latest_inbound_for(person) ].compact if sample.empty?
    !Contacts::SenderKind.service?(sample, provider_hints: false)
  end

  def owner_person?(person, contacts)
    return true if person.relationship_type == "self"

    owner = readable_accounts.map { |account| account.email_address.to_s.downcase }
    contacts.any? { |contact| owner.include?(contact.email.to_s.downcase) }
  end

  # People::Priority's inputs for one person, all from rows the primed standing
  # and the eligibility query already loaded — no queries.
  def person_facts(person, standing)
    People::Priority.facts_for(
      standing: standing,
      threads: people_standing.threads_for(person),
      contacts: person.contacts.to_a,
      latest_inbound: people_standing.latest_inbound_for(person),
      relationship_type: person.relationship_type,
      last_activity: person.last_email_at,
      now: list_now
    )
  end

  # An organization ranks by the people its standing is composed from: their
  # relationship evidence adds up, and the org's own newest mail keeps it live
  # even when that sample is quiet.
  def org_facts(org, standing, facts_by_person, last_activity)
    members = people_standing.sampled_people_for(org)
    member_facts = members.map { |member| facts_by_person[member.id] || person_facts(member, people_standing.person(member)) }
    facts = People::Priority.merge_facts(member_facts, standing: standing) ||
            People::Priority.facts_for(standing: standing, threads: [], contacts: [], latest_inbound: nil,
                                       relationship_type: nil, last_activity: nil, now: list_now)
    facts.with(last_activity: [ facts.last_activity, last_activity ].compact.max)
  end

  # One Scout-standing service per request, shared by every person/org row so the
  # user-scoped work (readable accounts, inbox folders, the awaiting-reply set) and
  # the batched per-person loads run once — not once per counterpart.
  def people_standing
    @people_standing ||= People::Standing.new(current_user, now: (@now ||= Time.current))
  end

  def prime_standing(people: [], organizations: [])
    people_standing.prime(people: people, organizations: organizations)
  end

  def eligible_persons
    person_ids = Contact.where(workspace_id: Current.workspace.id, sender_kind: Contact.sender_kinds[:person])
                        .where("email_count > 0")
                        .where.not(person_id: nil)
                        .select(:person_id)
    scope = Current.workspace.people.where(id: person_ids).includes(:contacts, :primary_organization)
    scope = filter_people(scope, @query) if @query.present?
    scope.to_a
  end

  def eligible_orgs
    org_ids = OrganizationMembership.joins(person: :contacts)
                                    .where(contacts: { workspace_id: Current.workspace.id })
                                    .where("contacts.email_count > 0")
                                    .select(:organization_id)
    scope = Current.workspace.organizations.where(id: org_ids)
    scope = scope.search(@query) if @query.present?
    scope.to_a
  end

  def filter_people(scope, query)
    like = "%#{Contact.sanitize_sql_like(query)}%"
    scope.where(
      "people.name ILIKE :like OR people.organization ILIKE :like OR " \
      "EXISTS (SELECT 1 FROM contacts c WHERE c.person_id = people.id AND (c.email ILIKE :like OR c.name ILIKE :like))",
      like: like
    )
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
      score: People::Priority.score(facts, now: list_now)
    )
  end

  # The busiest contact's address, read from the already-loaded contacts (the list
  # and detail paths both preload them) so the row costs no extra query; only an
  # unloaded single record falls back to Person#primary_email's ordered query.
  def person_primary_email(person)
    return person.primary_email unless person.contacts.loaded?

    person.contacts.max_by { |contact| contact.email_count.to_i }&.email
  end

  def org_counterpart(org, facts_by_person = {})
    people_count = org.active_people.joins(:contacts)
                      .where(contacts: { sender_kind: Contact.sender_kinds[:person] }).distinct.count
    services_count = org.contacts.kind_service.count
    standing = people_standing.organization(org)
    last_activity = org.email_messages.maximum(:received_at)
    facts = org_facts(org, standing, facts_by_person, last_activity)

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
      score: People::Priority.score(facts, now: list_now)
    )
  end

  def org_subtitle(people_count, services_count)
    parts = [ t("people.index.organization") ]
    parts << t("people.index.people_count", count: people_count) if people_count.positive?
    parts << t("people.index.services_count", count: services_count) if services_count.positive?
    parts.join(" · ")
  end

  # ── Streams count for the segmented control ("Streams · N") ────────────────
  def streams_count
    @streams_count ||= tag_groups_service.build_groups(inbox_folder_ids).size
  end

  # ── Shared inbox scoping (mirrors EmailMessagesController) ─────────────────
  def readable_accounts
    @readable_accounts ||= current_user.readable_email_accounts.ordered.to_a
  end

  def readable_account_ids
    @readable_account_ids ||= readable_accounts.map(&:id)
  end

  def inbox_folder_ids
    @inbox_folder_ids ||= Emails::InboxFolders.ids_for(readable_accounts)
  end

  def tag_groups_service
    @tag_groups_service ||= Emails::TagGroups.new(Current.workspace, readable_account_ids)
  end

  # Enqueue the workspace sender-kind backfill (debounced) when contacts with mail
  # still lack a verdict — so a workspace that predates the People place fills in
  # without a manual rake run.
  def lazy_backfill_sender_kinds
    return unless Contact.where(workspace_id: Current.workspace.id, sender_kind_source: nil)
                         .where("email_count > 0").exists?

    Contacts::SenderKindBackfillJob.enqueue_for(Current.workspace.id)
  end
end

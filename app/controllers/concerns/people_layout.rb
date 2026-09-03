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
  # member contact that has mail), each carrying its Scout standing. Need-you
  # first (by urgency), then Recent (by last activity, paginated + infinite scroll).
  def build_people_list
    now = Time.current
    @query = params[:q].to_s.strip
    lazy_backfill_sender_kinds

    counterparts = eligible_person_counterparts(now) + eligible_org_counterparts(now)
    need_you, recent = counterparts.partition(&:needs_you?)

    @need_you = need_you.sort_by { |c| [ -c.overdue_days, -activity_epoch(c) ] }
    recent_sorted = recent.sort_by { |c| -activity_epoch(c) }
    @recent_pagy, @recent = pagy_array(recent_sorted, limit: PEOPLE_PER_PAGE)
  end

  def activity_epoch(counterpart) = counterpart.last_activity&.to_i || 0

  def eligible_person_counterparts(now)
    person_ids = Contact.where(workspace_id: Current.workspace.id, sender_kind: Contact.sender_kinds[:person])
                        .where("email_count > 0")
                        .where.not(person_id: nil)
                        .select(:person_id)
    scope = Current.workspace.people.where(id: person_ids).includes(:contacts, :primary_organization)
    scope = filter_people(scope, @query) if @query.present?
    scope.map { |person| person_counterpart(person, now) }
  end

  def eligible_org_counterparts(now)
    org_ids = OrganizationMembership.joins(person: :contacts)
                                    .where(contacts: { workspace_id: Current.workspace.id })
                                    .where("contacts.email_count > 0")
                                    .select(:organization_id)
    scope = Current.workspace.organizations.where(id: org_ids).includes(:active_people)
    scope = scope.search(@query) if @query.present?
    scope.map { |org| org_counterpart(org, now) }
  end

  def filter_people(scope, query)
    like = "%#{Contact.sanitize_sql_like(query)}%"
    scope.where(
      "people.name ILIKE :like OR people.organization ILIKE :like OR " \
      "EXISTS (SELECT 1 FROM contacts c WHERE c.person_id = people.id AND (c.email ILIKE :like OR c.name ILIKE :like))",
      like: like
    )
  end

  def person_counterpart(person, now)
    People::Counterpart.new(
      kind: :person,
      record: person,
      name: person.display_name,
      subtitle: person.organization_name.presence,
      avatar_email: person.primary_email.presence || person.display_name,
      avatar_initial: nil,
      last_activity: person.last_email_at,
      standing: People::Standing.for_person(person, user: current_user, now: now)
    )
  end

  def org_counterpart(org, now)
    people_count = org.active_people.joins(:contacts)
                      .where(contacts: { sender_kind: Contact.sender_kinds[:person] }).distinct.count
    services_count = org.contacts.kind_service.count

    People::Counterpart.new(
      kind: :organization,
      record: org,
      name: org.name,
      subtitle: org_subtitle(people_count, services_count),
      avatar_email: nil,
      avatar_initial: org.name.to_s[0].to_s.upcase.presence || "?",
      last_activity: org.email_messages.maximum(:received_at),
      standing: People::Standing.for_organization(org, user: current_user, now: now)
    )
  end

  def org_subtitle(people_count, services_count)
    parts = [ t("people.index.organization"), t("people.index.people_count", count: people_count) ]
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

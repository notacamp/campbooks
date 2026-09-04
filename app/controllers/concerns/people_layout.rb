# frozen_string_literal: true

require "pagy/extras/array" # PeopleController#build_conversation paginates thread ids
require "pagy/extras/countless"

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
  # Reads the materialized people_standings table for the current user. The table
  # is refreshed in the background by People::StandingsRefreshJob; on first visit
  # (or right after deploy) it is populated inline. The page costs one paginated
  # read — no email_messages or email_threads queries at request time.
  def build_people_list
    @query = params[:q].to_s.strip
    lazy_backfill_sender_kinds          # cheap EXISTS; enqueues if unclassified contacts remain
    ensure_standings_fresh              # inline on first visit, enqueue-and-serve when stale
    rows = PeopleStanding.for_user(current_user).ranked
    rows = rows.search(@query) if @query.present?

    # Build lane structure: Need-you rows grouped by verb, then Recent paginated.
    needing = rows.needing.map(&:to_counterpart)
    @lanes  = build_lanes(needing)
    # Keep @need_you for backward-compat (detail pane, specs).
    @need_you = needing

    @recent_pagy, recent_rows = pagy_countless(rows.recent, limit: PEOPLE_PER_PAGE)
    @recent = recent_rows.map(&:to_counterpart)
  end

  # Group Need-you counterparts into ordered verb lanes.
  # Returns array of { verb:, label:, counterparts: [] }.
  def build_lanes(need_you)
    order = %i[reply decide pay chase nudge]
    by_verb = need_you.group_by { |cp| cp.standing.verb }

    order.filter_map do |verb|
      counterparts = by_verb[verb]
      next if counterparts.blank?

      { verb: verb, label: t("people.index.lanes.#{verb}"), counterparts: counterparts }
    end
  end

  # ── Standings freshness ────────────────────────────────────────────────────
  def ensure_standings_fresh
    if People::Standings.missing?(current_user)
      # First visit (or first visit after this deploy): compute inline, once.
      return unless Contact.where(workspace_id: Current.workspace.id).where("email_count > 0").exists?

      People::Standings.refresh!(current_user)
    elsif People::Standings.stale?(current_user)
      # Rows exist but are old: serve what we have and refresh in the background.
      People::StandingsRefreshJob.enqueue_for(current_user.id)
    end
  end

  # ── Streams count for the segmented control ("Streams · N") ────────────────
  def streams_count
    @streams_count ||= Rails.cache.fetch("people_streams_count_#{current_user.id}", expires_in: 1.hour) do
      tag_groups_service.build_groups(inbox_folder_ids).size
    end
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

# frozen_string_literal: true

require "pagy/extras/countless"

# The rethought "bold" home (the "Now" surface): the same materialized home feed,
# reframed as a queue of decisions you clear. Scout's ledger up top, the segment
# rings, the decision deck (a stack of the existing Feed::Card renders), setup
# cards, the cleared moment, and Scout's log over workspace Events — with the
# docked Scout bar (Campbooks::ScoutBar).
#
# It reuses HomeController's collaborators (Feed::Reader, Home::InboxState,
# Feed::RefreshJob) rather than duplicating the pipeline. Gated on the readiness
# FLAG alone (require_bold_layout_enabled), NOT the per-user preference — a
# classic-mode user can open /now on a flag-on build. The Scout overlay is a
# follow-up PR; this page's bar is just a link to /scout.
class NowController < ApplicationController
  include Pagy::Backend

  before_action :require_bold_layout_enabled

  # Deliberately generous: the deck clears one card at a time, so a page holds a
  # good run before the controller lazily fetches more.
  PAGE_SIZE = 12

  SEGMENTS = %i[all priority follow_ups mail time].freeze

  # Which FeedItem kinds each non-priority segment gathers. `all` is every kind;
  # `priority` is the attention cluster (any kind, attention: true) rather than a
  # kind list, so it's handled specially. A `notice` (an actionable notification)
  # is always attention, so it counts under All and Priority but belongs to no kind
  # segment here — it's not a follow-up, a piece of mail, or a time item.
  SEGMENT_KINDS = {
    follow_ups: %w[follow_up reply_reminder],
    mail:       %w[email_action starred_email tag_suggestion late_receivable],
    time:       %w[calendar_event reminder task]
  }.freeze

  def index
    @segment = resolve_segment
    @reader = Feed::Reader.new(current_user)

    # A freshly-connected mailbox mid-first-scan gets the same full-screen "Scout
    # is reading your inbox" stage Home shows, rather than an empty deck.
    @first_sync = Onboarding::FirstSyncStatus.new(current_user)
    if @first_sync.stage? && session[:first_sync_skipped] != current_user.id.to_s
      render "home/first_sync" and return
    end

    @inbox_state = Home::InboxState.for(current_user)
    @doc_review_count = Current.workspace ? Current.workspace.documents.needs_review.count : 0
    @segment_counts = segment_counts

    # The deck's timeline leg (paginated); the attention leg is always shown whole
    # on page 1, ahead of it.
    @pagy, timeline_items = pagy_countless(segment_timeline_scope, limit: PAGE_SIZE)
    @timeline_pairs = @reader.present(timeline_items)

    # A ?page=N fetch (the deck's lazy load-more) only ever needs the next timeline
    # page appended — attention, rings, ledger and log stay put.
    return render_deck_page if params[:page].present?

    @attention_pairs = segment_attention_pairs
    @setup_items = deck_setup_items
    # The active segment's kinds, for the deck controller's live-append filter
    # (empty for all/priority — those are decided by name on the client).
    @segment_kinds = SEGMENT_KINDS.fetch(@segment, [])
    @ledger = Now::Ledger.new(current_user, need_you: @segment_counts[:priority])
    @log = Now::Log.new(current_user)

    Feed::RefreshJob.enqueue_for(current_user.id) if @reader.stale?
  rescue Pagy::OverflowError
    # The deck asked for a page past the end (the feed can shrink between requests).
    # End the deck rather than 500.
    @pagy = nil
    @timeline_pairs = []
    render_deck_page
  end

  # Reverse one of Scout's logged actions from the deck's log, in place: archive →
  # unarchive, tag → remove_tag. Reuses the exact EmailActions the palette/feed use
  # (so the permission gate and inbox broadcast are identical), then swaps the log
  # row to a muted "Undone" state. Only reversible system events are offered an
  # Undo (see Campbooks::Now::LogRow); everything else is Open-only.
  def undo_log
    event = accessible_system_event(params[:id])
    return head :not_found unless event

    result = reverse_event(event)

    respond_to do |format|
      format.turbo_stream do
        if result[:success]
          render turbo_stream: [
            turbo_stream.replace(
              ActionView::RecordIdentifier.dom_id(event),
              render_to_string(Campbooks::Now::LogRow.new(event: event, undone: true), layout: false)
            ),
            notify_stream(result[:message] || t(".undone"))
          ]
        else
          render turbo_stream: notify_stream(result[:message] || t(".failed"), severity: :error),
                 status: :unprocessable_entity
        end
      end
      format.html { redirect_to now_path }
    end
  end

  private

  def resolve_segment
    seg = params[:segment].to_s.to_sym
    SEGMENTS.include?(seg) ? seg : :all
  end

  # One page of the deck's timeline leg, as a turbo-stream append + a refreshed
  # pagination-state node the now-deck controller reads for the next fetch.
  def render_deck_page
    render "now/index", formats: :turbo_stream
  end

  # The attention (needs-you) cards for the current segment, presented and filtered.
  def segment_attention_pairs
    pairs = @reader.attention
    return pairs if @segment == :all || @segment == :priority

    kinds = SEGMENT_KINDS.fetch(@segment, [])
    pairs.select { |pair| kinds.include?(pair[:item].kind) }
  end

  # The ranked timeline (ambient) leg for the current segment, as a relation to
  # paginate. `priority` is attention-only, so it has no timeline leg.
  def segment_timeline_scope
    return current_user.feed_items.none if @segment == :priority

    scope = @reader.timeline_scope
    return scope if @segment == :all

    scope.where(kind: SEGMENT_KINDS.fetch(@segment, []))
  end

  # Counts for the segment rings: one grouped pass over the user's ACTIVE feed
  # items, mapped to segments (priority = the attention subset). Docs rides its own
  # review-queue count.
  def segment_counts
    active = current_user.feed_items.active
    by_kind = active.group(:kind).count
    kind = ->(*names) { names.sum { |n| by_kind[n].to_i } }

    {
      all:        by_kind.values.sum,
      priority:   active.attention.count,
      follow_ups: kind.call("follow_up", "reply_reminder"),
      mail:       kind.call("email_action", "starred_email", "tag_suggestion", "late_receivable"),
      time:       kind.call("calendar_event", "reminder", "task"),
      docs:       @doc_review_count
    }
  end

  # Incomplete-and-not-dismissed setup items — the same source shared/_setup_banner
  # uses. Rendered as deck cards after the feed items (on the `all` segment only:
  # setup isn't a follow-up or a bit of mail). Empty unless there's a workspace.
  def deck_setup_items
    return [] unless @segment == :all && Current.workspace

    dismissed = Array(Current.workspace.settings["dismissed_setup_keys"]).map(&:to_s)
    SetupStatus.new(Current.workspace).incomplete_items
               .reject { |item| dismissed.include?(item[:key].to_s) }
  end

  def accessible_system_event(id)
    return nil unless Current.workspace

    Current.workspace.events.accessible_to(current_user).where(actor_id: nil).find_by(id: id)
  end

  def reverse_event(event)
    case event.name
    when "email.archived"
      return not_reversible unless event.subject.is_a?(EmailMessage)

      EmailActions.run("unarchive", email_message: event.subject, args: {}, user: current_user)
    when "email.tagged"
      tag_name = event.payload["tag"]
      return not_reversible unless event.subject.is_a?(EmailMessage) && tag_name.present?

      EmailActions.run("remove_tag", email_message: event.subject, args: { tag_name: tag_name }, user: current_user)
    else
      not_reversible
    end
  end

  def not_reversible
    { success: false, message: t(".not_reversible") }
  end
end

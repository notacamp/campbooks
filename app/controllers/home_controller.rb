# frozen_string_literal: true

require "pagy/extras/countless"

# The Instagram-style home: story-rings (lazy Skim tray) above an infinite,
# multi-source feed of things to address. A small "needs attention" cluster is
# pinned on top; the rest is a reverse-chronological timeline reaching back as
# far as there's anything actionable.
#
# The feed is a materialized spine (feed_items) populated by Feed::Source classes
# — see Feed::Reader / Feed::Generator. This controller only reads it: fast, and
# paginated with pagy_countless (no COUNT) behind a lazy turbo-frame sentinel,
# mirroring the email thread list.
class HomeController < ApplicationController
  # Deliberately small: the feed favors a few large, breathing items per screen
  # over a dense list. More load lazily as you scroll.
  PAGE_SIZE = 8

  def index
    @reader = Feed::Reader.new(current_user)
    # Past the curated spine, the same infinite scroll falls into Rewind — the
    # scroll-back through past highlights. A keyset cursor (?before=) or the first
    # rewind page (?rewind=1) routes here instead of re-reading feed_items.
    return render_rewind if rewind_request?

    # A freshly-connected mailbox whose first scan hasn't completed gets the
    # full-screen "Scout is reading your inbox" stage instead of an empty feed.
    @first_sync = Onboarding::FirstSyncStatus.new(current_user)
    if @first_sync.stage?
      render :first_sync and return
    end

    @attention = @reader.attention
    @pagy, items = pagy_countless(@reader.timeline_scope, limit: PAGE_SIZE)
    @timeline = @reader.present(items)
    # Coalesce adjacent filing suggestions into one tight tag queue.
    @timeline_groups = Feed::Reader.group_timeline(@timeline)
    # How many things still want the user — drives the orienting subtitle. Count
    # what the feed can actually surface (presented attention + this page), NOT raw
    # feed_item rows: present() drops items a refresh hasn't reconciled yet, so a
    # raw count would promise "N things" over a visibly empty feed. Extra pages
    # behind the lazy sentinel can't be cheaply totalled — nudge into the "lots"
    # tier rather than undercount.
    shown = @attention.size + @timeline.size
    @pending_count = @pagy&.next ? [ shown, 10 ].max : shown
    # End of the curated spine → hand the scroll off to Rewind instead of
    # dead-ending, but only when there are past highlights to surface.
    @show_rewind = @pagy&.next.nil? && Feed::Rewind.new(current_user).any?
    # Documents awaiting human sign-off — surfaced as the "Documents" skim ring
    # in the rings strip (the doc-review queue lives there now, not the timeline).
    @doc_review_count = Current.workspace ? Current.workspace.documents.needs_review.count : 0
    # Only when the feed is empty: distinguish the zero-states so the copy points
    # the right way. "Disconnected" is its own state — a user with inboxes that
    # have all gone inactive must NOT be told to "connect your inbox" as if brand new.
    if @attention.empty? && @timeline.empty?
      accounts = Current.workspace&.email_accounts
      @inbox_state =
        if accounts&.active&.exists?
          :caught_up        # a live inbox; the queue is genuinely clear
        elsif accounts&.exists?
          :disconnected     # connected before, but every inbox is disconnected
        else
          :none             # brand-new — never connected an inbox
        end
    end

    # Keep the spine fresh without blocking the render — a debounced background
    # refresh that the event hooks would otherwise have triggered.
    Feed::RefreshJob.enqueue_for(current_user.id) if @reader.stale?
  rescue Pagy::OverflowError
    # The lazy infinite-scroll sentinel asked for a page past the end. The feed
    # can shrink between requests (items get acted on / refreshed away), so this
    # is the end of the feed, not a 500: retire the sentinel for the turbo-stream
    # fetch, or fall back to the first page for a direct ?page=N visit.
    end_of_feed
  end

  private

  # A request for Rewind: the first rewind page (?rewind=1) or a keyset cursor
  # into older highlights (?before=…&before_id=…&period=…).
  def rewind_request?
    params[:rewind].present? || params[:before].present?
  end

  # One page of Rewind, as a turbo stream: this leg's time chapters + highlight
  # cards plus the advanced (or retired) sentinel. Lazy by construction — only the
  # page the user scrolled to is queried, so reaching back years costs nothing
  # until then.
  def render_rewind
    rewind = Feed::Rewind.new(current_user, before: Feed::Rewind.cursor_from_params(params))
    @rewind_entries = rewind.entries
    @rewind_next = rewind.next_cursor
    render :rewind, formats: :turbo_stream
  end

  def end_of_feed
    respond_to do |format|
      format.turbo_stream do
        @pagy = nil
        @timeline_groups = []
        render :index
      end
      format.html { redirect_to root_path }
    end
  end
end

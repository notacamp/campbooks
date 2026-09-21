# frozen_string_literal: true

require "pagy/extras/countless"

module Api
  module App
    module Now
      # GET /api/app/now
      # The Now deck: segment counts, attention cluster, paginated timeline,
      # ledger, log, inbox state. Mirrors NowController#index (read path only).
      class DeckController < Api::App::BaseController
        PAGE_SIZE = 12

        SEGMENTS = %i[all priority follow_ups mail time].freeze
        SEGMENT_KINDS = {
          follow_ups: %w[follow_up reply_reminder reply_owed],
          mail:       %w[email_action starred_email tag_suggestion late_receivable late_payable],
          time:       %w[calendar_event reminder task]
        }.freeze

        def index
          segment = resolve_segment
          reader  = ::Feed::Reader.new(current_user)

          doc_review_count = current_workspace ? current_workspace.documents.needs_review.count : 0
          segment_counts   = build_segment_counts(doc_review_count)

          # Timeline leg (paginated).
          timeline_scope = build_timeline_scope(reader, segment)
          pagy, timeline_items = pagy_countless(timeline_scope, limit: PAGE_SIZE)
          timeline_pairs = reader.present(timeline_items)

          # Attention cluster (whole) — only on first page or when not paginating.
          attention_pairs = if params[:page].blank?
            build_attention_pairs(reader, segment)
          else
            []
          end

          ledger = ::Now::Ledger.new(current_user, need_you: segment_counts[:priority])
          log    = ::Now::Log.new(current_user)
          inbox_state = ::Home::InboxState.for(current_user)  # returns a symbol

          ::Feed::RefreshJob.enqueue_for(current_user.id) if reader.stale?

          render_data(
            Api::App::NowDeckSerializer.new(
              segment:        segment,
              segment_counts: segment_counts,
              attention_pairs: attention_pairs,
              timeline_pairs:  timeline_pairs,
              timeline_pagy:   pagy,
              ledger:          ledger,
              log:             log,
              inbox_state:     inbox_state
            ).as_json
          )
        end

        private

        def resolve_segment
          seg = params[:segment].to_s.to_sym
          SEGMENTS.include?(seg) ? seg : :all
        end

        def build_segment_counts(doc_review_count)
          active  = current_user.feed_items.active
          by_kind = active.group(:kind).count
          kind    = ->(*names) { names.sum { |n| by_kind[n].to_i } }

          {
            all:        by_kind.values.sum,
            priority:   active.attention.count,
            follow_ups: kind.call("follow_up", "reply_reminder", "reply_owed"),
            mail:       kind.call("email_action", "starred_email", "tag_suggestion", "late_receivable", "late_payable"),
            time:       kind.call("calendar_event", "reminder", "task"),
            docs:       doc_review_count
          }
        end

        def build_timeline_scope(reader, segment)
          return current_user.feed_items.none if segment == :priority

          scope = reader.timeline_scope
          return scope if segment == :all

          scope.where(kind: SEGMENT_KINDS.fetch(segment, []))
        end

        def build_attention_pairs(reader, segment)
          pairs = reader.attention
          return pairs if segment == :all || segment == :priority

          kinds = SEGMENT_KINDS.fetch(segment, [])
          pairs.select { |pair| kinds.include?(pair[:item].kind) }
        end
      end
    end
  end
end

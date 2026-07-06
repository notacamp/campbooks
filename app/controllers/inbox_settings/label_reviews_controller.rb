# frozen_string_literal: true

module InboxSettings
  # Renders the pending provider-label review panel and processes the batched
  # Map / Keep / Ignore decisions. Renders into the `inbox_settings_panel` Turbo
  # Frame alongside the rest of the inbox-settings panels.
  class LabelReviewsController < BaseController
    def index
      @pending = LabelImportDecision
                 .for_workspace(Current.workspace)
                 .pending_review
                 .includes(:email_account)
                 .order(:provider_label_name)
      @workspace_tags = Current.workspace.tags.visible.order(:name)
    end

    # PATCH /inbox_settings/label_reviews/bulk_decide
    # Processes a batch of review decisions submitted from the review form. Each
    # row carries { decision: "mapped"|"kept"|"ignored", tag_id?: "..." } keyed
    # by label_import_decision_id. Silently skips already-resolved rows.
    def bulk_decide
      decisions = params[:decisions].to_unsafe_h  # { id => { decision:, tag_id?: } }

      decisions.each do |id, attrs|
        process_decision(id, attrs)
      rescue => e
        Rails.logger.error("[InboxSettings::LabelReviews] decision #{id}: #{e.message}")
      end

      # Re-render the panel — if nothing is left pending, show the empty state.
      @pending = LabelImportDecision
                 .for_workspace(Current.workspace)
                 .pending_review
                 .includes(:email_account)
                 .order(:provider_label_name)
      @workspace_tags = Current.workspace.tags.visible.order(:name)

      remaining = @pending.count
      flash_msg  = remaining.zero? ? t(".all_done") : t(".saved_count", count: decisions.size)

      render turbo_stream: [
        turbo_stream.update("label_reviews_list", partial: "inbox_settings/label_reviews/list",
                            locals: { pending: @pending, workspace_tags: @workspace_tags }),
        notify_stream(flash_msg)
      ]
    end

    private

    def process_decision(id, attrs)
      row = LabelImportDecision.for_workspace(Current.workspace).find(id)
      return unless row.decision_pending?

      decision = attrs[:decision].to_s
      case decision
      when "mapped"
        tag = Current.workspace.tags.find_by(id: attrs[:tag_id])
        return unless tag

        link_tag_to_account(tag, row)
        row.resolve!(decision: :mapped, tag: tag, reviewed_by: Current.user)

      when "kept"
        # The external tag already exists — just link it and mark kept.
        external_tag = row.email_account.external_tags
                          .find_by(external_label_id: row.provider_label_id)
        link_tag_to_account(external_tag, row) if external_tag
        row.resolve!(decision: :kept, tag: external_tag, reviewed_by: Current.user)

      when "ignored"
        row.resolve!(decision: :ignored, reviewed_by: Current.user)
      end
    end

    # Upsert a TagAccountLink for the given tag + the account recorded in the
    # decision row. Idempotent — a link that already exists is a no-op.
    def link_tag_to_account(tag, decision_row)
      return unless tag

      TagAccountLink.find_or_create_by!(
        tag_id:           tag.id,
        email_account_id: decision_row.email_account_id
      ) do |link|
        link.provider_label_id   = decision_row.provider_label_id
        link.provider_label_name = decision_row.provider_label_name
      end
    rescue ActiveRecord::RecordNotUnique, ActiveRecord::RecordInvalid
      # Already linked — that is fine.
    end
  end
end

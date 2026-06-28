# frozen_string_literal: true

# Builds the kanban columns for a pipeline board and gates which items the
# current user may see. Shared by PipelineBoardController (initial render) and
# PipelineMembershipsController (re-render after add/remove).
module PipelineColumns
  extend ActiveSupport::Concern

  COLUMN_LIMIT = 50

  private

  def build_columns(pipeline)
    pipeline.stages.ordered.map do |stage|
      memberships = pipeline.memberships
        .in_stage(stage.id)
        .includes(:item)
        .order(last_moved_at: :desc, id: :desc)
        .limit(COLUMN_LIMIT + 1)
        .to_a
        .select { |m| m.item && accessible_item?(m.item) }

      {
        stage: stage,
        memberships: memberships.first(COLUMN_LIMIT),
        has_more: memberships.size > COLUMN_LIMIT,
        draggable: !stage.is_terminal?
      }
    end
  end

  # Documents are workspace-scoped; emails are gated to the accounts the current
  # user may read. Never reveal an email from an account this user can't access.
  def accessible_item?(item)
    return true unless item.is_a?(EmailMessage)

    readable_account_ids.include?(item.email_account_id)
  end

  def readable_account_ids
    @readable_account_ids ||= Current.user.readable_email_accounts.pluck(:id).to_set
  end
end

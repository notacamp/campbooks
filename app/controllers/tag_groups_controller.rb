# frozen_string_literal: true

# Bulk "clear the group" actions on a tag group's drill-in view. Grouping itself
# is configured under Settings -> Tags (assign a group name to any tag); this
# controller only mutates mail, delegating to the security-scoped bulk tools. The
# group name arrives as a form param (not a path segment) so names with spaces or
# "&" (e.g. "Newsletters & promos") round-trip cleanly.
class TagGroupsController < ApplicationController
  before_action :require_authentication
  before_action :validate_group!

  def archive_all
    count = Emails::TagGroupBulkAction.new(Current.user, params[:group]).archive_all
    redirect_to email_messages_path, success: t(".archived", count: count)
  end

  def mark_all_read
    count = Emails::TagGroupBulkAction.new(Current.user, params[:group]).mark_all_read
    redirect_to email_messages_path(group: params[:group]), success: t(".marked_read", count: count)
  end

  private

  def validate_group!
    return if params[:group].present? &&
              Tag.where(workspace_id: Current.user.workspace_id, group_name: params[:group]).exists?

    head :bad_request
  end
end

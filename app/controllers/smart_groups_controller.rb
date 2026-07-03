# Bulk "clear the bucket" actions on a smart group's drill-in view. Settings
# (which buckets bundle) live in InboxSettings::SmartGroupsController — this
# controller only mutates mail, delegating to the security-scoped bulk tools.
class SmartGroupsController < ApplicationController
  before_action :require_authentication
  before_action :validate_bucket!

  def archive_all
    count = Emails::SmartGroupBulkAction.new(Current.user, params[:bucket]).archive_all
    redirect_to email_messages_path, success: t(".archived", count: count)
  end

  def mark_all_read
    count = Emails::SmartGroupBulkAction.new(Current.user, params[:bucket]).mark_all_read
    redirect_to email_messages_path(smart_group: params[:bucket]), success: t(".marked_read", count: count)
  end

  private

  def validate_bucket!
    head :bad_request unless User::SMART_GROUP_BUCKETS.include?(params[:bucket])
  end
end

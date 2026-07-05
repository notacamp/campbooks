# frozen_string_literal: true

# Shows a single DigestIssue. Scoped through the current user's digests so
# a foreign digest_id + issue_id pair 404s per the invisible-resource rule.
class DigestIssuesController < ApplicationController
  before_action :require_authentication
  before_action :require_digests_enabled
  before_action :set_issue

  def show
  end

  private

  def set_issue
    @digest = current_user.scheduled_digests.find(params[:digest_id])
    @issue  = @digest.issues.find(params[:id])
  rescue ActiveRecord::RecordNotFound
    head :not_found
  end
end

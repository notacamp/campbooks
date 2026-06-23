# Workspace activity feed: a retrospective, read-only timeline of domain Events.
# Distinct from the home feed (which is prospective — "what to do now"); this is
# the historical record of what has happened. Scoped to the workspace and gated
# by Event.accessible_to so a member never sees email/calendar events for
# mailboxes they can't read.
class ActivityController < ApplicationController
  def index
    scope = Current.workspace.events.accessible_to(current_user).recent
    scope = scope.where(name: params[:name]) if params[:name].present?
    scope = scope.where(name: group_event_names(params[:group])) if params[:group].present?

    @group = params[:group]
    @pagy, @events = pagy(scope, items: 30)

    respond_to do |format|
      format.html
      format.turbo_stream # pagination append -> index.turbo_stream.erb
    end
  end

  private

  # The registered event keys belonging to a group (e.g. :email), so a group
  # filter matches every event type in that group.
  def group_event_names(group)
    Events::Registry.all.select { |d| d.group.to_s == group.to_s }.map(&:key)
  end
end

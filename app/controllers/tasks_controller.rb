# frozen_string_literal: true

# The Tasks pages — the board, list, skim triage, task page, task form and task
# discussions — retired into Time (asks). A task is an ask now: it lives on Now (its
# email card, or a decision card) and Time (agenda rows + the "No date yet"
# section), with no page of its own.
#
# The Task record, the public API (/api/v1/tasks) and the MCP tools are unchanged;
# only the web UI moved. These two actions survive so old links — notifications,
# digests, bookmarks — keep working by redirecting to the Time surface. Still gated
# by Features.tasks? (404 when off) and requires authentication.
class TasksController < ApplicationController
  before_action :require_authentication
  before_action :require_tasks_enabled

  def index
    redirect_to time_path
  end

  # Land on the day the ask sits on (in the user's zone) when it has a due date, so
  # the row is in view; otherwise the agenda's default (today), where an undated ask
  # shows under "No date yet". A task the user can't access falls back to /time too.
  def show
    task = Task.accessible_to(current_user).find_by(id: params[:id])
    date = task&.due_at&.in_time_zone(current_user.effective_time_zone)&.to_date
    redirect_to time_path(date: date&.iso8601)
  end
end

class Current < ActiveSupport::CurrentAttributes
  attribute :session
  attribute :workspace
  # Web requests resolve the user from the session cookie. Background contexts
  # (Scout reply jobs, the email-reply daemon) have no session, so they set
  # acting_user explicitly — letting tools gate on Current.user uniformly,
  # exactly as controllers do. Left nil on the web, so request behavior is unchanged.
  attribute :acting_user
  # Scopes granted to the API credential serving this request (Array<String>).
  # Set by Api::McpController so scope-aware MCP tool handlers (get_overview,
  # get_setup_status) can trim their output to the sections the caller may see.
  attribute :api_scopes

  # Request-local memo for Time::BusyIntervals — the calendar/focus-block busy set
  # keyed by [user_id, from.to_i, to.to_i], so a deck of ask cards previewing hold
  # slots runs the busy query once per window instead of once per card. Reset per
  # request/job by CurrentAttributes; lazily initialized to {} on first use.
  attribute :busy_intervals_cache

  def user
    acting_user || session&.user
  end
end

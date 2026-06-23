class Current < ActiveSupport::CurrentAttributes
  attribute :session
  attribute :workspace
  # Web requests resolve the user from the session cookie. Background contexts
  # (Scout reply jobs, the email-reply daemon) have no session, so they set
  # acting_user explicitly — letting tools gate on Current.user uniformly,
  # exactly as controllers do. Left nil on the web, so request behavior is unchanged.
  attribute :acting_user

  def user
    acting_user || session&.user
  end
end

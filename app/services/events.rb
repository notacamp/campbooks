# The domain-event bus. `Events.publish` is the single entry point domain code
# uses to record that something happened; everything else (the activity feed,
# workflow triggers) reads from the resulting Event rows.
#
#   Events.publish("document.approved", subject: document, actor: Current.user,
#                  payload: { type: document.classification&.name })
#
# Background callers (jobs, where Current.workspace may be unset) should pass
# `workspace:` explicitly. See Events::Publisher for the full contract.
module Events
  def self.publish(name, **options)
    Publisher.call(name, **options)
  end
end

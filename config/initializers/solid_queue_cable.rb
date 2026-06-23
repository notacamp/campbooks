# Ensure the SolidCable database connection is established after
# Solid Queue forks worker processes. Without this, broadcasts
# (Turbo Streams, notifications) inside jobs fail with
# "The `cable` database is not configured" in development.
if defined?(SolidQueue) && defined?(SolidCable)
  SolidQueue.on_start do
    SolidCable::Record.connection
  rescue ActiveRecord::AdapterNotSpecified
    # Cable database not configured — broadcasts will fail silently
  end
end

# frozen_string_literal: true

# The user's IANA time zone (e.g. "Europe/Lisbon"), captured once from the device
# by the local-greeting Stimulus controller when it is still unset. Drives the
# bold Time surface's day bucketing and focus-block slot finding. Nil falls back
# to the workspace's primary calendar zone, then UTC (see User#effective_time_zone).
class AddTimeZoneToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :time_zone, :string
  end
end

# frozen_string_literal: true

# Per-user home/navigation layout preference for the opt-in "bold" rethink
# (Now / People / Paper / Money / Time + the Now page). 0 = classic (today's
# Home/Mail/Calendar/Scout/Files), 1 = bold. Only takes effect on a build with
# Features.bold_layout? on; classic is the safe default for everyone.
class AddLayoutModeToUsers < ActiveRecord::Migration[8.1]
  def change
    add_column :users, :layout_mode, :integer, default: 0, null: false
  end
end

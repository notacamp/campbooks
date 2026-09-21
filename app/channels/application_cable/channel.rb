# frozen_string_literal: true

# Base class for the app's own ActionCable channels. The app previously had none
# (live updates rode turbo-rails' Turbo::StreamsChannel), so this was never
# generated — UserSyncChannel, the /api/app SPA fan-out channel, is the first.
module ApplicationCable
  class Channel < ActionCable::Channel::Base
  end
end

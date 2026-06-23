# frozen_string_literal: true

module Emails
  # Pushes a freshly-built Skim tray to a user's Turbo Stream so the inbox "feed"
  # stays live — counts drop as mail is addressed, and new mail appears without a
  # manual reload. Mirrors the app's existing per-user stream pattern
  # (e.g. "notifications_#{user.id}"); the inbox subscribes via turbo_stream_from
  # "skim_#{user.id}".
  #
  # Replaces the inner #skim_tray_content (not the turbo-permanent #skim_tray frame
  # itself), so the stable lazy frame is preserved while its contents refresh.
  class SkimTrayBroadcaster
    def self.refresh(user)
      new(user).refresh
    end

    # Refresh every user who can read the account (e.g. after new mail lands).
    def self.refresh_account(account)
      EmailAccountUser.where(email_account: account).includes(:user).filter_map(&:user).uniq.each do |user|
        refresh(user)
      end
    end

    def initialize(user)
      @user = user
    end

    def refresh
      return unless @user

      rings = Emails::SkimDeck.for(
        @user,
        now: Time.current,
        whitelist_mode: @user.workspace&.whitelist_mode?,
        memory: Emails::SkimActionMemory.new(@user)
      )
      html = ApplicationController.render(partial: "skim/tray_content", locals: { rings: rings })

      Turbo::StreamsChannel.broadcast_replace_to(
        "skim_#{@user.id}",
        target: "skim_tray_content",
        html: html
      )
    rescue => e
      Rails.logger.error("[Emails::SkimTrayBroadcaster] #{e.class}: #{e.message}")
    end
  end
end

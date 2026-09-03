# frozen_string_literal: true

module Feed
  # Streams newly-generated feed items into an open Now page's decision deck, live.
  #
  # "Motion is state": when Scout materializes a card for mail that just needs you,
  # it slides into the BACK of the deck — appended to #feed_timeline via the
  # per-user "now_#{user.id}" Turbo stream the page subscribes to — and the
  # now-deck controller ticks the counter and pulses the back sheets (or rises it
  # as the new top when the deck was cleared). Segment-awareness and de-duplication
  # live on the client (each card carries data-feed-kind / -attention / -live), so
  # this only needs to render and append.
  #
  # Called at the end of Feed::RefreshJob#perform with that run's inserted items;
  # a no-op when there are none. Renders through ApplicationController.render (never
  # Phlex#call in a job — see reference_phlex_render_in_jobs) in the recipient's
  # locale, exactly like Emails::InboxBroadcaster.
  class LiveDeck
    def self.broadcast(user, items)
      new(user, items).broadcast
    end

    def initialize(user, items)
      @user = user
      @items = Array(items)
    end

    def broadcast
      return if @user.nil? || @items.empty?

      # Present drops any item whose source now says it's invalid (raced with a
      # handling in another tab) — the same safety net the page render uses.
      pairs = Feed::Reader.new(@user).present(@items)
      return if pairs.empty?

      I18n.with_locale(locale) do
        pairs.each { |pair| append(pair) }
      end
    rescue => e
      Rails.logger.error("[Feed::LiveDeck] #{e.class}: #{e.message}")
    end

    private

    def append(pair)
      html = ApplicationController.render(
        Campbooks::Feed::Card.new(item: pair[:item], subject: pair[:subject], live: true),
        layout: false
      )
      Turbo::StreamsChannel.broadcast_append_to("now_#{@user.id}", target: "feed_timeline", html: html)
    end

    def locale
      @user.locale.presence || I18n.default_locale
    end
  end
end

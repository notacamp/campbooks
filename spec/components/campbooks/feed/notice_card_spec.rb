# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::Feed::NoticeCard, type: :component do
  def render_card(notification, **item_attrs)
    item = FeedItem.new(
      id: SecureRandom.uuid,
      kind: "notice",
      data: {},
      generated_at: Time.current,
      created_at: notification.created_at || 1.hour.ago
    )
    ApplicationController.render(described_class.new(item: item, subject: notification), layout: false)
  end

  it "renders the notification title, label, and time" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :system,
      priority: :action_required,
      title: "Inbox disconnected",
      body: "Your Gmail account needs to be reconnected.",
      link_url: "/email_accounts/123",
      group_key: "account_disconnected_123",
      count: 1,
      created_at: 2.hours.ago
    )

    html = render_card(notification)

    expect(html).to include("Inbox disconnected")
    expect(html).to include("Your Gmail account needs to be reconnected.")
    expect(html).to include(I18n.t("components.feed.notice_card.label"))
  end

  it "shows Reconnect as the primary verb for account_disconnected notifications" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :system,
      priority: :action_required,
      title: "Inbox disconnected",
      link_url: "/email_accounts/1",
      group_key: "account_disconnected_1",
      count: 1,
      created_at: 1.hour.ago
    )

    html = render_card(notification)

    expect(html).to include(I18n.t("components.feed.notice_card.reconnect"))
  end

  it "shows Review as the primary verb for document_review notifications" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :document,
      priority: :action_required,
      title: "Documents need review",
      link_url: "/documents?filter=needs_review",
      group_key: "document_review_pending",
      count: 1,
      created_at: 1.hour.ago
    )

    html = render_card(notification)

    expect(html).to include(I18n.t("components.feed.notice_card.review"))
  end

  it "shows the generic Open label for other categories" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :task,
      priority: :action_required,
      title: "Task overdue",
      link_url: "/tasks/42",
      group_key: "task_overdue_42",
      count: 1,
      created_at: 1.hour.ago
    )

    html = render_card(notification)

    expect(html).to include(I18n.t("components.feed.notice_card.open"))
  end

  it "renders Done and Later action buttons" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :system,
      priority: :action_required,
      title: "Something",
      link_url: "/somewhere",
      group_key: "system_thing",
      count: 1,
      created_at: 1.hour.ago
    )

    html = render_card(notification)

    expect(html).to include(I18n.t("components.feed.notice_card.done"))
    expect(html).to include(I18n.t("components.feed.notice_card.later"))
  end

  it "omits the Open link when link_url is blank" do
    notification = Notification.new(
      id: SecureRandom.uuid,
      category: :system,
      priority: :action_required,
      title: "No link notice",
      link_url: nil,
      group_key: "system_no_link",
      count: 1,
      created_at: 1.hour.ago
    )

    html = render_card(notification)

    expect(html).not_to include(I18n.t("components.feed.notice_card.open"))
  end
end

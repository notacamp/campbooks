# frozen_string_literal: true

# Preview for the feed cards' "peek inside" disclosure
# (Campbooks::Feed::ExpandablePreview) and its server half
# (Campbooks::Feed::EmailPreviewFrame). The collapsed states render the real
# component; the frame states render the response content directly, since the
# lazy fetch has no live feed item to hit from a preview. In-memory records
# (id set so the path helpers resolve), mirroring FeedHighlightCardComponentPreview.
class FeedExpandablePreviewComponentPreview < ViewComponent::Preview
  # Collapsed, as it sits under a card's Scout note ("Show email").
  def collapsed
    render Campbooks::Feed::ExpandablePreview.new(item: item)
  end

  # Collapsed with the reminder/task wording ("Show source email").
  def collapsed_source_label
    render Campbooks::Feed::ExpandablePreview.new(
      item: item,
      label: I18n.t("components.feed.expandable_preview.show_source")
    )
  end

  # The frame content once loaded — plain-text fallback body (no HTML).
  def loaded_frame
    render Campbooks::Feed::EmailPreviewFrame.new(item: item, subject: message)
  end

  # The frame when the message is gone or no longer accessible.
  def unavailable_frame
    render Campbooks::Feed::EmailPreviewFrame.new(item: item, subject: nil)
  end

  private

  def item
    FeedItem.new(id: 9001)
  end

  def message
    EmailMessage.new(
      id: 9001,
      from_address: '"Maple Lodge" <billing@maplelodge.example>',
      subject: "Invoice #2025-114",
      summary: "The March invoice is attached — total €1,240, due by the 28th. " \
               "Let us know if the billing address needs updating before we file it.",
      received_at: Time.utc(2024, 9, 12, 9, 41)
    )
  end
end

# frozen_string_literal: true

# Previews for Campbooks::Feed::TagSuggestionCard.
# Notice variant (applied): Scout auto-filed the email, offering one "Undo".
# Legacy variant (unapplied): ask-style "File it / Not now" for in-flight items.
class FeedTagSuggestionCardPreview < ViewComponent::Preview
  # Notice mode — Scout filed the email; user sees "Filed … under #tag" + Undo.
  def notice
    workspace = Workspace.first || Workspace.new(name: "Demo")
    user = workspace.users.first || User.new(name: "Demo")
    account = workspace.email_accounts.first || EmailAccount.new(workspace: workspace)

    email = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      subject: "Invoice for December services",
      from_address: "billing@acme.com",
      received_at: 2.hours.ago
    )

    item = FeedItem.new(
      id: SecureRandom.uuid,
      kind: "tag_suggestion",
      subject: email,
      data: { "tag_name" => "invoices", "applied" => true },
      sort_at: Time.current
    )

    render Campbooks::Feed::TagSuggestionCard.new(item: item, subject: email)
  end

  # Legacy ask-mode — older card asking the user to confirm the filing.
  def ask
    workspace = Workspace.first || Workspace.new(name: "Demo")
    user = workspace.users.first || User.new(name: "Demo")
    account = workspace.email_accounts.first || EmailAccount.new(workspace: workspace)

    email = EmailMessage.new(
      id: SecureRandom.uuid,
      email_account: account,
      subject: "Your Q4 contract renewal — action needed",
      from_address: "contracts@supplier.com",
      received_at: 1.day.ago
    )

    item = FeedItem.new(
      id: SecureRandom.uuid,
      kind: "tag_suggestion",
      subject: email,
      data: { "tag_name" => "contracts", "applied" => false },
      sort_at: Time.current
    )

    render Campbooks::Feed::TagSuggestionCard.new(item: item, subject: email)
  end
end

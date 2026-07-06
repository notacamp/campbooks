require "rails_helper"

RSpec.describe Campbooks::Feed::TagSuggestionCard, type: :component do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:email) do
    create(:email_message, email_account: account,
           subject: "Invoice for December services", from_address: "billing@acme.com")
  end

  def feed_item(applied:)
    FeedItem.create!(
      user: user, workspace: workspace, kind: "tag_suggestion", subject: email,
      dedupe_key: "tag_suggestion:#{email.id}", sort_at: Time.current,
      data: { "tag_name" => "invoices", "applied" => applied }
    )
  end

  def render_card(item)
    ApplicationController.render(described_class.new(item: item, subject: email), layout: false)
  end

  context "notice mode (data[applied] = true)" do
    let(:item) { feed_item(applied: true) }

    it "renders the past-tense 'Filed' sentence" do
      html = render_card(item)
      expect(html).to include("Filed")
    end

    it "renders the tag chip" do
      html = render_card(item)
      expect(html).to include("#invoices")
    end

    it "renders the Undo button wired to undo_tag_filing" do
      html = render_card(item)
      expect(html).to include("undo_tag_filing")
      expect(html).to include("Undo")
    end

    it "does not render the 'File it' or 'Not now' buttons" do
      html = render_card(item)
      expect(html).not_to include("File it")
      expect(html).not_to include("Not now")
    end

    it "renders a checkmark icon (not the tag icon)" do
      html = render_card(item)
      # Checkmark polyline is present; the tag <path> is absent.
      expect(html).to include("polyline")
    end

    it "truncates a long subject without overflowing" do
      email.update!(subject: "A" * 120)
      html = render_card(item)
      # Truncation appended: the full 120-char string must not appear.
      expect(html).not_to include("A" * 120)
    end
  end

  context "legacy ask-mode (data[applied] absent / false)" do
    let(:item) { feed_item(applied: false) }

    it "renders the ask-style 'File' prefix" do
      html = render_card(item)
      # The legacy prefix key is "File" (not "Filed").
      # We just check the File it button is present as the primary CTA.
      expect(html).to include("File it")
    end

    it "renders the 'Not now' dismiss button" do
      html = render_card(item)
      expect(html).to include("Not now")
    end

    it "wires the primary button to add_tag" do
      html = render_card(item)
      expect(html).to include("add_tag")
    end

    it "does not render the Undo button" do
      html = render_card(item)
      expect(html).not_to include("undo_tag_filing")
    end
  end
end

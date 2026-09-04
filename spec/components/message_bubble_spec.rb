# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::MessageBubble, type: :component do
  let(:account) { create(:email_account, email_address: "me@myco.example") }
  let(:message) do
    create(:email_message, email_account: account, from_address: "sofia@brightloop.example",
           subject: "Q3 deck", body: "Please review the deck.", channel: "email")
  end

  def render(component)
    ApplicationController.render(component, layout: false)
  end

  describe ":chat (the reading-pane bubble)" do
    it "renders the directional bubble with the author and body" do
      html = render(described_class.new(message: message, sent: false, variant: :chat, expanded: true))
      expect(html).to include("thread-msg")
      expect(html).to include("thread-bubble")
      expect(html).to include("sofia@brightloop.example")
      expect(html).to include("Please review the deck")
    end

    it "shows the selected badge when selected" do
      html = render(described_class.new(message: message, sent: false, variant: :chat, selected: true))
      expect(html).to include("Selected")
    end
  end
end

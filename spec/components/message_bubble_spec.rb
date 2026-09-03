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

  describe ":flow (the People conversation block)" do
    it "renders the channel chip and body" do
      html = render(described_class.new(message: message, sent: false, variant: :flow, channel_chip: true, full: true))
      expect(html).to include("Email") # channel chip
      expect(html).to include("Please review the deck")
    end

    it "labels an outbound message You and takes the passed name" do
      sent = create(:email_message, email_account: account, from_address: account.email_address, body: "On it.", channel: "email")
      html = render(described_class.new(message: sent, sent: true, variant: :flow, name: "You", channel_chip: true, full: true))
      expect(html).to include(">You<")
    end

    it "renders an Open thread link" do
      threaded = create(:email_message, email_account: account, email_thread: create(:email_thread, email_account: account),
                        from_address: "sofia@brightloop.example", body: "hi", channel: "email")
      html = render(described_class.new(message: threaded, sent: false, variant: :flow, open_thread: true, full: false))
      expect(html).to include("Open thread")
    end
  end
end

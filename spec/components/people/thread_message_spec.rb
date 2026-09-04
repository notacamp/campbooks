# frozen_string_literal: true

require "rails_helper"

RSpec.describe Campbooks::People::ThreadMessage, type: :component do
  let(:account) { create(:email_account, email_address: "me@myco.example") }
  let(:thread) { create(:email_thread, email_account: account, subject: "Q3 deck") }
  let(:contact) { create(:contact, email: "sofia@brightloop.example", name: "Sofia Martins", email_account: account) }

  def render_msg(message, open: false, full_body: false, lazy_src: nil, content_only: false)
    ApplicationController.render(
      described_class.new(
        message: message,
        person_first_name: "Sofia",
        open: open,
        full_body: full_body,
        lazy_src: lazy_src,
        content_only: content_only
      ),
      layout: false
    )
  end

  it "renders a closed <details> when open is false" do
    msg = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "sofia@brightloop.example", body: "Please review.")
    html = render_msg(msg, open: false)
    expect(html).to match(/<details[^>]*class="group/)
    expect(html).not_to match(/<details[^>]*open[^>]*>/)
  end

  it "renders an open <details> when open is true" do
    msg = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "sofia@brightloop.example", body: "Please review.")
    html = render_msg(msg, open: true)
    expect(html).to match(/<details[^>]*open/)
  end

  it "shows You for a sent message" do
    sent = create(:email_message, email_account: account, email_thread: thread, contact: nil,
                  from_address: "me@myco.example", body: "On it.")
    html = render_msg(sent, open: true)
    expect(html).to include(">You<")
    expect(html).to include("to Sofia")
  end

  it "shows the contact display name for an inbound message" do
    msg = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "sofia@brightloop.example", body: "A question.")
    html = render_msg(msg, open: true)
    expect(html).to include("Sofia Martins")
    expect(html).to include("to you")
  end

  it "shows the address local-part titleized when there is no contact name" do
    nameless_contact = create(:contact, email: "alice.smith@example.com", email_account: account)
    msg = create(:email_message, email_account: account, email_thread: thread, contact: nameless_contact,
                 from_address: "alice.smith@example.com", body: "Hello.")
    html = render_msg(msg, open: true)
    expect(html).to include("Alice Smith")
  end

  it "renders an attachments row when files are present" do
    msg = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "sofia@brightloop.example", body: "See attached.")
    msg.files.attach(io: StringIO.new("fake pdf"), filename: "report.pdf", content_type: "application/pdf")

    html = render_msg(msg, open: true)
    expect(html).to include("report.pdf")
  end

  it "renders no-content placeholder when body and summary are blank" do
    msg = create(:email_message, email_account: account, email_thread: thread, contact: contact,
                 from_address: "sofia@brightloop.example", body: nil, summary: nil)
    html = render_msg(msg, open: true)
    expect(html).to include("No content")
  end

  describe "lazy body" do
    let(:msg) do
      create(:email_message, email_account: account, email_thread: thread, contact: contact,
             from_address: "sofia@brightloop.example", body: "Earlier note. #{'y' * 200} TAILMARKER")
    end

    it "renders the summary line over a lazy frame instead of the body when lazy_src is set" do
      html = render_msg(msg, lazy_src: "/people/p1/messages/#{msg.id}")
      expect(html).to match(/<turbo-frame[^>]*id="people_message_#{msg.id}"[^>]*src="\/people\/p1\/messages\/#{msg.id}"[^>]*loading="lazy"/)
      expect(html).to include("Earlier note.")
      expect(html).not_to include("TAILMARKER")
    end

    it "renders just the body (no <details>) with content_only" do
      html = render_msg(msg, content_only: true)
      expect(html).not_to include("<details")
      expect(html).not_to include("<turbo-frame")
      expect(html).to include("TAILMARKER")
    end
  end
end

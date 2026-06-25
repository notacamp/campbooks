require "rails_helper"

# Campbooks::EmailScoutActions brings Scout's read + suggested actions to the
# OPEN email (the drawer and the full detail pane only — never the inbox list).
# The contract worth pinning: the reply chip is a draft_reply POST carrying the
# surface (so its preview lands in that surface's compose slot), gated on send
# permission. Appearance is covered by the Lookbook preview + Playwright.
RSpec.describe Campbooks::EmailScoutActions, type: :component do
  def render_for(surface:, can_send: true, **opts)
    account = EmailAccount.new(id: 1, email_address: "me@example.com", color: "#000000")
    message = EmailMessage.new(
      id: 1,
      from_address: "emma@example.com",
      subject: "Invoice",
      ai_action_prompt: "Emma needs sign-off by Friday.",
      ai_suggested_actions: [
        { "tool" => "add_tag", "args" => { "tag_name" => "invoices" } },
        { "tool" => "archive" }
      ]
    )
    message.email_account = account
    ApplicationController.render(described_class.new(message: message, surface: surface, can_send: can_send, **opts), layout: false)
  end

  it "renders Scout's read line" do
    expect(render_for(surface: :drawer)).to include("Emma needs sign-off")
  end

  it "posts draft_reply carrying the drawer surface" do
    html = render_for(surface: :drawer)
    expect(html).to include("tool=draft_reply")
    expect(html).to include("surface=drawer")
    expect(html).to include("Suggest reply")
  end

  it "posts draft_reply carrying the detail surface" do
    html = render_for(surface: :detail)
    expect(html).to include("tool=draft_reply")
    expect(html).to include("surface=detail")
  end

  it "hides the reply affordance when the user cannot send" do
    html = render_for(surface: :drawer, can_send: false)
    expect(html).not_to include("draft_reply")
    # but other suggested actions still surface
    expect(html).to include("tool=add_tag")
  end

  it "renders the other suggested actions through ChatActions" do
    html = render_for(surface: :drawer)
    expect(html).to include("tool=add_tag")
    expect(html).to include("tool=archive")
  end

  it "shows the AI provenance when the email's summary was AI-generated" do
    account = EmailAccount.new(id: 3, email_address: "me@example.com")
    message = EmailMessage.new(id: 3, ai_action_prompt: "Read me",
                               ai_provenance: { "provider" => "mistral", "model" => "x", "region" => "EU" })
    message.email_account = account
    html = ApplicationController.render(described_class.new(message: message, surface: :drawer), layout: false)
    expect(html).to include("Processed by")
    expect(html).to include("EU")
  end

  it "renders nothing when there is no read, no actions, and no send permission" do
    account = EmailAccount.new(id: 2, email_address: "me@example.com")
    message = EmailMessage.new(id: 2, ai_suggested_actions: [])
    message.email_account = account
    html = ApplicationController.render(
      described_class.new(message: message, surface: :drawer, can_send: false), layout: false
    )
    expect(html.strip).to be_empty
  end
end

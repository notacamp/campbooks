require "rails_helper"

RSpec.describe Campbooks::Now::LogRow, type: :component do
  include Rails.application.routes.url_helpers
  let(:workspace) { create(:workspace) }

  def render_row(event, undone: false)
    ApplicationController.render(described_class.new(event: event, undone: undone), layout: false)
  end

  it "offers Undo AND Open for a reversible email.archived event on an accessible message" do
    email = create(:email_message)
    event = create(:event, workspace: workspace, name: "email.archived", subject: email,
                   occurred_at: 1.hour.ago, payload: { "subject" => email.subject })

    html = render_row(event)

    expect(html).to include(now_log_undo_path(event))                  # Undo form posts here
    expect(html).to include(email_message_path(email))                 # Open link
    expect(html).to include(I18n.t("components.now.log_row.undo"))
    expect(html).to include(I18n.t("components.now.log_row.open"))
  end

  it "offers only Open (no Undo) for a non-reversible event like document.processed" do
    doc = create(:document, workspace: workspace)
    event = create(:event, workspace: workspace, name: "document.processed", subject: doc,
                   occurred_at: 1.hour.ago, payload: { "filename" => "invoice.pdf" })

    html = render_row(event)

    expect(html).not_to include("/undo")                                # no Undo form
    expect(html).to include(document_path(doc))                         # Open link present
  end

  it "renders the muted 'Undone' state with no actions when undone" do
    email = create(:email_message)
    event = create(:event, workspace: workspace, name: "email.archived", subject: email, occurred_at: 1.hour.ago)

    html = render_row(event, undone: true)

    expect(html).to include(I18n.t("components.now.log_row.undone"))
    expect(html).not_to include(now_log_undo_path(event))               # undo form gone
  end

  it "shows the event's HH:MM timestamp in mono, tabular figures" do
    event = create(:event, workspace: workspace, name: "email.archived",
                   occurred_at: Time.zone.local(2026, 9, 3, 14, 5))

    html = render_row(event)
    expect(html).to include("14:05")
    expect(html).to include("font-mono").and include("tabular-nums")
  end
end

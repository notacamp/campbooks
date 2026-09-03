require "rails_helper"

# The bold-layout Engine anatomy (compose from intent). Classic rendering is
# covered by the existing email_compose specs; here we assert the bold-only
# pieces: the intent input, the chip envelope, the persistent saved status, and
# the AI-gated tone footer.
RSpec.describe Campbooks::Compose::Engine, type: :component do
  def render_bold(ai: true, **overrides)
    allow_any_instance_of(ApplicationController).to receive(:ai_provider_available?).and_return(ai)
    args = {
      shell: :desk, bold: true, persistent_status: true, mode: :new_message,
      action_url: "/email_messages/send_new", accounts: [], signatures: [],
      heading: "New message", back_url: "/email_messages"
    }.merge(overrides)
    ApplicationController.render(described_class.new(**args), layout: false)
  end

  it "renders the intent input, chip envelope and a bordered editor card" do
    html = render_bold

    expect(html).to include('data-controller="compose-intent"')
    expect(html).to include('name="to_address"')
    expect(html).to include('name="subject"')
    # subject looks like a chip: a secondary-filled rounded container
    expect(html).to match(/bg-secondary[^"]*rounded-lg|rounded-lg[^"]*bg-secondary/)
    expect(html).to include("rounded-2xl border border-border")
  end

  it "keeps the saved status persistent with a check glyph" do
    html = render_bold

    expect(html).to include('data-compose-autosave-persistent-status-value="true"')
    expect(html).to include('data-compose-autosave-saved-text-value="Saved just now"')
    expect(html).to include('data-compose-autosave-target="statusIcon"')
    expect(html).to include('data-compose-autosave-target="status"')
  end

  it "shows the Shorter and Warmer tone buttons when a text provider is available" do
    html = render_bold(ai: true)

    expect(html).to include('data-compose-engine-tone-param="shorter"')
    expect(html).to include('data-compose-engine-tone-param="warmer"')
    expect(html).to include("Shorter")
    expect(html).to include("Warmer")
    expect(html).to include("click->compose-engine#rewriteDraft")
    expect(html).to include('data-compose-engine-rewrite-url-value')
  end

  it "hides the tone buttons when no text provider is available" do
    html = render_bold(ai: false)

    expect(html).not_to include("compose-engine#rewriteDraft")
    expect(html).not_to include('data-compose-engine-tone-param')
  end

  it "marks an inferred recipient and subject" do
    html = render_bold(to: "Sofia Martins <sofia@example.com>", to_inferred: true,
                       subject: "Re: Q3 deck", subject_inferred: true)

    expect(html).to include('data-contact-pill-input-inferred-value="true"')
    expect(html).to include('data-compose-engine-target="subjectInferred"')
    expect(html).to include("input->compose-engine#clearSubjectInferred")
  end

  it "does not mark fields inferred when they were not" do
    html = render_bold(to_inferred: false, subject_inferred: false)

    expect(html).not_to include('data-contact-pill-input-inferred-value="true"')
    expect(html).not_to include('data-compose-engine-target="subjectInferred"')
  end

  it "renders the Scout's-draft mark hidden, ready to reveal on body-set" do
    html = render_bold

    expect(html).to include('data-compose-engine-target="scoutChip"')
    expect(html).to include("compose-chat:body-set@window->compose-engine#markScoutDraft")
    expect(html).to include('data-scout-draft="false"')
  end
end

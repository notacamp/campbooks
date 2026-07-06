require "rails_helper"

RSpec.describe ContactAnalysisJob, type: :job do
  let(:contact) { create(:contact, :with_emails, messages_count: 5) }

  describe "#perform" do
    let(:analyzer) { instance_double(Ai::ContactAnalyzer) }

    before do
      # The workspace has a text provider set up (otherwise analysis is skipped).
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(true)
      allow(Ai::ContactAnalyzer).to receive(:new).with(contact, user_prompt: nil).and_return(analyzer)
      allow(analyzer).to receive(:analyze!)
    end

    it "calls ContactAnalyzer on the contact" do
      described_class.perform_now(contact.id)

      expect(Ai::ContactAnalyzer).to have_received(:new).with(contact, user_prompt: nil)
      expect(analyzer).to have_received(:analyze!).with(force: false)
    end

    it "skips recently analyzed contacts" do
      contact.update!(analyzed_at: 5.days.ago)

      described_class.perform_now(contact.id)
      expect(analyzer).not_to have_received(:analyze!)
    end

    it "re-analyzes when forced" do
      contact.update!(analyzed_at: 5.days.ago)

      described_class.perform_now(contact.id, force: true)
      expect(analyzer).to have_received(:analyze!).with(force: true)
    end

    it "handles missing contact gracefully" do
      expect {
        described_class.perform_now(-1)
      }.not_to raise_error
    end

    it "skips analysis when no text AI provider is configured" do
      allow(Ai::ProviderSetup).to receive(:configured?).and_return(false)

      described_class.perform_now(contact.id)
      expect(Ai::ContactAnalyzer).not_to have_received(:new)
    end
  end

  # ── Integration: Current.workspace regression + provider resolution ──────────
  #
  # A name odd enough that Contacts::Consolidator never fuzzy-matches it against
  # people created by other tests running in the same database.
  ANALYZED_NAME = "Zorbal Quexley"

  FAKE_ANALYSIS = {
    name: ANALYZED_NAME,
    organization: "Acme GmbH",
    relationship_type: "client",
    context_summary: "Zorbal handles invoicing at Acme GmbH.",
    communication_patterns: {
      typical_topics: [ "invoicing" ], tone: "formal",
      urgency_level: "medium", primary_role: "accounts payable contact"
    }
  }.to_json.freeze

  let(:integration_ws) { Workspace.create!(name: "Contact Analysis WS") }
  let(:integration_contact) do
    integration_ws.contacts.create!(email: "zorbal@acme.example", email_count: 6)
  end

  before do
    adapter = integration_ws.ai_adapters.create!(name: "Text AI provider", provider: "openai", api_key: "test-key", enabled: true)
    integration_ws.ai_configurations.create!(
      purpose: "email_analysis", ai_adapter: adapter, enabled: true,
      model: "gpt-4o-mini", max_tokens: 500, temperature: 0.0
    )
    account = EmailAccount.create!(
      workspace: integration_ws, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    5.times do |i|
      account.email_messages.create!(
        provider_message_id: "m-#{SecureRandom.hex(6)}", contact: integration_contact,
        subject: "Invoice #{i}", body: "Please find invoice #{i} attached.",
        received_at: i.hours.ago
      )
    end
  end

  # Stub the adapter seam only: Ai::Configuration.for must still resolve the
  # config through Current.workspace — that resolution IS the regression under
  # test (the job used to leave Current.workspace unset, so no provider resolved
  # and analysis silently no-oped forever on the cloud).
  def with_fake_text_adapter(response_text)
    fake = Object.new
    fake.define_singleton_method(:chat) { |**| response_text }
    original = AiAdapter.instance_method(:adapter_instance)
    AiAdapter.send(:define_method, :adapter_instance) { fake }
    original_key = ENV.delete("ANTHROPIC_API_KEY") # keep the legacy fallback from masking a regression
    yield
  ensure
    AiAdapter.send(:define_method, :adapter_instance, original)
    ENV["ANTHROPIC_API_KEY"] = original_key if original_key
  end

  it "analyzes through the workspace's configured provider and materializes the organization" do
    with_fake_text_adapter(FAKE_ANALYSIS) do
      described_class.perform_now(integration_contact.id)
    end

    integration_contact.reload
    expect(integration_contact.analyzed_at).to be_present,
      "analysis never completed — Current.workspace regression: no provider resolves inside the job"
    person = integration_contact.person
    expect(person.read_attribute(:organization)).to eq("Acme GmbH")

    org = integration_ws.organizations.find_by(name: "Acme GmbH")
    expect(org).to be_present, "organization should materialize automatically after analysis"
    expect(org.organization_memberships.active.exists?(person_id: person.id)).to be true

    expect(Current.workspace).to be_nil, "job must reset Current.workspace"
  end

  it "throttles concurrency so a backlog can't storm the AI provider" do
    expect(described_class.concurrency_limit).to eq(2)
    expect(described_class.concurrency_key).to be_present
  end

  it "a rate-limited attempt is retried with backoff instead of losing the contact" do
    fake = Object.new
    fake.define_singleton_method(:chat) { |**| raise Faraday::TooManyRequestsError, "429" }
    original = AiAdapter.instance_method(:adapter_instance)
    AiAdapter.send(:define_method, :adapter_instance) { fake }
    original_key = ENV.delete("ANTHROPIC_API_KEY")

    expect {
      described_class.perform_now(integration_contact.id)
    }.to have_enqueued_job(described_class)

    expect(integration_contact.reload.analyzed_at).to be_nil
    expect(Current.workspace).to be_nil
  ensure
    AiAdapter.send(:define_method, :adapter_instance, original)
    ENV["ANTHROPIC_API_KEY"] = original_key if original_key
  end

  it "resets Current.workspace even when the analyzer raises" do
    original = Ai::ContactAnalyzer.instance_method(:analyze!)
    Ai::ContactAnalyzer.send(:define_method, :analyze!) { |force: false| raise "boom" }
    begin
      described_class.perform_now(integration_contact.id)
    rescue StandardError
      nil
    ensure
      Ai::ContactAnalyzer.send(:define_method, :analyze!, original)
    end

    expect(Current.workspace).to be_nil
  end
end

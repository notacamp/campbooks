# frozen_string_literal: true

require "test_helper"

class ContactAnalysisJobTest < ActiveSupport::TestCase
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

  setup do
    @ws = Workspace.create!(name: "Contact Analysis WS")
    adapter = @ws.ai_adapters.create!(name: "Text AI provider", provider: "openai", api_key: "test-key", enabled: true)
    @ws.ai_configurations.create!(
      purpose: "email_analysis", ai_adapter: adapter, enabled: true,
      model: "gpt-4o-mini", max_tokens: 500, temperature: 0.0
    )
    account = EmailAccount.create!(
      workspace: @ws, email_address: "box-#{SecureRandom.hex(4)}@example.com",
      provider: :google, refresh_token: "tok", active: true
    )
    @contact = @ws.contacts.create!(email: "zorbal@acme.example", email_count: 6)
    5.times do |i|
      account.email_messages.create!(
        provider_message_id: "m-#{SecureRandom.hex(6)}", contact: @contact,
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

  test "analyzes through the workspace's configured provider and materializes the organization" do
    with_fake_text_adapter(FAKE_ANALYSIS) do
      ContactAnalysisJob.perform_now(@contact.id)
    end

    @contact.reload
    assert @contact.analyzed_at.present?,
      "analysis never completed — Current.workspace regression: no provider resolves inside the job"
    person = @contact.person
    assert_equal "Acme GmbH", person.read_attribute(:organization)

    org = @ws.organizations.find_by(name: "Acme GmbH")
    assert org.present?, "organization should materialize automatically after analysis"
    assert org.organization_memberships.active.exists?(person_id: person.id)

    assert_nil Current.workspace, "job must reset Current.workspace"
  end

  test "resets Current.workspace even when the analyzer raises" do
    original = Ai::ContactAnalyzer.instance_method(:analyze!)
    Ai::ContactAnalyzer.send(:define_method, :analyze!) { |force: false| raise "boom" }
    begin
      ContactAnalysisJob.perform_now(@contact.id)
    rescue StandardError
      nil
    ensure
      Ai::ContactAnalyzer.send(:define_method, :analyze!, original)
    end

    assert_nil Current.workspace
  end
end

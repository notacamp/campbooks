require "rails_helper"

# Managed "Campbooks AI" adapters must be granted by the workspace's plan. The gate
# is permissive on every cloud plan today (the billing lever is the deferred
# managed-AI usage quota); this proves the seam blocks once a plan drops managed_ai.
RSpec.describe AiAdapter, "managed entitlement", type: :model do
  let(:workspace) { create(:workspace, plan: "free") }

  def managed_adapter(ws)
    ws.ai_adapters.build(name: "Campbooks AI — Text", provider: "mistral", managed: true, enabled: true)
  end

  it "allows a managed adapter when the plan grants managed_ai" do
    expect(managed_adapter(workspace)).to be_valid
  end

  it "blocks a managed adapter when managed_ai is overridden off" do
    workspace.update!(entitlement_overrides: { "managed_ai" => { "allowed" => false } })
    adapter = managed_adapter(workspace)
    expect(adapter).not_to be_valid
    expect(adapter.errors.details[:managed]).to include(a_hash_including(error: :plan_upgrade_required))
  end

  it "leaves BYO-key adapters unaffected by the managed gate" do
    workspace.update!(entitlement_overrides: { "managed_ai" => { "allowed" => false } })
    byo = workspace.ai_adapters.build(name: "My OpenAI", provider: "openai", managed: false, api_key: "sk-test")
    expect(byo).to be_valid
  end
end

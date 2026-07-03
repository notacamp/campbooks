require "rails_helper"

# The learning ConfidenceGuard + provenance Tasks::Builder applies when a
# Learning::Memory is injected. Without a memory it behaves exactly as before.
RSpec.describe "Tasks::Builder learning guard" do
  let(:workspace) { create(:workspace) }
  let(:contact)   { create(:contact, workspace: workspace) }
  let(:sender)    { create(:email_message, from_address: "hi@acme.com", contact_id: contact.id) }

  def memory
    Learning::Memory.new(source: Learning::Sources::Tasks.new(workspace))
  end

  def item(overrides = {})
    { "title" => "Send the signed contract", "description" => "Sign and return.",
      "priority" => "normal", "confidence" => 0.9 }.merge(overrides)
  end

  def build(mem: memory)
    Tasks::Builder.call(workspace: workspace, source: sender, raw_items: [ item ], learning_memory: mem)
  end

  def seed(n, status:, domain: "acme.com", contact_id: contact.id)
    n.times do
      email = create(:email_message, from_address: "hi@#{domain}", contact_id: contact_id)
      Task.create!(workspace: workspace, source: email, title: "old task",
                   status: status, priority: :normal, confidence: 0.9, ai_suggested: true)
    end
  end

  it "suppresses a new task when the sender's suggested tasks are consistently cancelled" do
    seed(3, status: :cancelled)

    expect { build }.not_to change(Task, :count)
  end

  it "stages the task when there is no learned signal" do
    expect { build }.to change(Task, :count).by(1)
  end

  it "never suppresses when no memory is injected" do
    seed(3, status: :cancelled)

    expect {
      Tasks::Builder.call(workspace: workspace, source: sender, raw_items: [ item ])
    }.to change(Task, :count).by(1)
  end

  it "records the learning provenance on a staged task (accepted consensus)" do
    seed(3, status: :done)

    task = build.first
    expect(task.reload.extracted_data["_learning"]).to include("label" => "accepted", "signal" => "contact", "count" => 3, "total" => 3)
  end
end

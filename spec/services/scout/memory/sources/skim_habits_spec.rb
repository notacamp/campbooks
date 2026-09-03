require "rails_helper"

RSpec.describe Scout::Memory::Sources::SkimHabits do
  let(:ws) { Workspace.create!(name: "SK WS", slug: "sk-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "sk-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }

  def skim(label, count, **cols)
    count.times { LearningDecision.create!(domain: "email_skim", user: user, workspace_id: ws.id, label: label, **cols) }
  end

  it "surfaces a domain-tier archive habit at >= 3 examples and >= 60% share" do
    skim("archive", 9, sender_domain: "newsletter.com")
    skim("keep", 3, sender_domain: "newsletter.com")

    entry = source.entries.find { |e| e.id.include?("skim:domain") }
    expect(entry.plain).to eq("You usually archive mail from @newsletter.com (9 of your last 12).")
    expect(entry.facet).to eq(:stack)
    expect(entry.origin).to eq(:learned)
    expect(entry.origin_detail).to eq("Learned from how you skim")
    expect(entry.actions).to eq(%i[confirm remove])
  end

  it "does not surface a key below threshold or without a >=60% winner" do
    skim("archive", 2, sender_domain: "small.com")           # too few
    skim("archive", 3, sender_domain: "split.com")           # 3 vs 3 -> no 60% winner
    skim("keep", 3, sender_domain: "split.com")
    ids = source.entries.map(&:id)
    expect(ids).not_to include(a_string_including("small.com"))
    expect(source.entries.select { |e| e.id.include?("split") }).to be_empty
  end

  it "resolves the contact tier to the contact name" do
    contact = ws.contacts.create!(email: "amy@acme.com", name: "Amy Vendor")
    skim("keep", 4, contact_id: contact.id)
    entry = source.entries.find { |e| e.id.include?("skim:contact") }
    expect(entry.plain).to eq("You usually keep mail from Amy Vendor (4 of your last 4).")
  end

  it "confirm records another supporting decision" do
    skim("archive", 9, sender_domain: "newsletter.com")
    skim("keep", 3, sender_domain: "newsletter.com")
    entry = source.entries.find { |e| e.id.include?("skim:domain") }
    expect { source.confirm(entry) }.to change(LearningDecision, :count).by(1)
    added = LearningDecision.where(domain: "email_skim", user_id: user.id).order(:created_at).last
    expect(added.signals).to eq("confirmed" => true)
    expect(added.label).to eq("archive")
  end

  it "remove forgets the habit (deletes the key's decisions)" do
    skim("archive", 9, sender_domain: "newsletter.com")
    skim("keep", 3, sender_domain: "newsletter.com")
    entry = source.entries.find { |e| e.id.include?("skim:domain") }
    expect { source.remove(entry) }.to change {
      LearningDecision.where(domain: "email_skim", user_id: user.id).where("LOWER(sender_domain) = ?", "newsletter.com").count
    }.from(12).to(0)
  end
end

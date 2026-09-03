require "rails_helper"

RSpec.describe Scout::Memory::Sources::DocumentHabits do
  let(:ws) { Workspace.create!(name: "DH WS", slug: "dh-#{SecureRandom.hex(4)}") }
  let(:user) { ws.users.create!(name: "T", email_address: "dh-#{SecureRandom.hex(4)}@example.com", password: "password123") }
  let(:source) { described_class.new(workspace: ws, user: user) }
  let(:invoice) { ws.document_types.create!(name: "invoice", color: "#7c5cfc") }

  def build_doc(sender, type_id, status)
    doc = ws.documents.new(document_type_id: type_id, review_status: status, ai_status: :completed,
      metadata: { "sender_name" => sender })
    doc.original_file.attach(io: StringIO.new("pdf"), filename: "doc.pdf", content_type: "application/pdf")
    doc.save!
    doc
  end

  def approved_doc(sender, type, _i = nil)
    build_doc(sender, type.id, :approved)
  end

  it "surfaces a learned sender -> type mapping once there are >= 3 approved docs" do
    3.times { |i| approved_doc("Cloudhost", invoice, i) }

    entry = source.entries.first
    expect(entry.plain).to eq("Documents from Cloudhost are usually Invoices.")
    expect(entry.facet).to eq(:filing)
    expect(entry.origin).to eq(:learned)
    expect(entry.origin_detail).to eq("Learned from 3 corrections")
    expect(entry.actions).to eq(%i[confirm])
    expect(entry.id).to start_with("dochabit:")
  end

  it "does not surface a sender below the 3-example threshold" do
    2.times { |i| approved_doc("Rare Vendor", invoice, i) }
    expect(source.entries).to be_empty
  end

  it "ignores non-approved documents" do
    3.times { build_doc("Pending Co", invoice.id, :pending) }
    expect(source.entries).to be_empty
  end

  it "confirm is an acknowledging no-op that succeeds" do
    3.times { |i| approved_doc("Cloudhost", invoice, i) }
    expect(source.confirm(source.entries.first)).to be_truthy
  end
end

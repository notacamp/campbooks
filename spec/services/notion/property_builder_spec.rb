require "rails_helper"

RSpec.describe Notion::PropertyBuilder do
  describe ".build" do
    it "maps scalar types via FieldMapper" do
      props = described_class.build({
        "Name" => { type: "title", value: "Invoice 42" },
        "Amount" => { type: "number", value: "99.5" },
        "Tags" => { type: "multi_select", value: %w[a b] }
      })

      expect(props.dig("Name", "title", 0, "text", "content")).to eq("Invoice 42")
      expect(props.dig("Amount", "number")).to eq(99.5)
      expect(props.dig("Tags", "multi_select").map { |m| m["name"] }).to eq(%w[a b])
    end

    it "skips read-only property types" do
      props = described_class.build({ "Created" => { type: "created_time", value: "x" } })
      expect(props).to be_empty
    end

    it "skips blanks except checkbox" do
      props = described_class.build({
        "Notes" => { type: "rich_text", value: "" },
        "Paid" => { type: "checkbox", value: "false" }
      })
      expect(props).not_to have_key("Notes")
      expect(props.dig("Paid", "checkbox")).to be(false)
    end

    it "builds a files property from uploaded file ids" do
      props = described_class.build({}, file_uploads: { "Attachment" => [ { id: "fu_1", name: "a.pdf" } ] })

      file = props.dig("Attachment", "files", 0)
      expect(file["type"]).to eq("file_upload")
      expect(file.dig("file_upload", "id")).to eq("fu_1")
      expect(file["name"]).to eq("a.pdf")
    end

    it "supports people and relation values" do
      props = described_class.build({
        "Owner" => { type: "people", value: "u1, u2" },
        "Linked" => { type: "relation", value: %w[p1] }
      })
      expect(props.dig("Owner", "people").map { |p| p["id"] }).to eq(%w[u1 u2])
      expect(props.dig("Linked", "relation").map { |r| r["id"] }).to eq(%w[p1])
    end
  end
end

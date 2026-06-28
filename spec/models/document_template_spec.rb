require "rails_helper"
RSpec.describe DocumentTemplate, type: :model do
  subject(:t) { build(:document_template) }
  it { is_expected.to belong_to(:workspace) }
  it { is_expected.to have_one_attached(:preview_pdf) }
  it { is_expected.to validate_presence_of(:name) }
  it { is_expected.to define_enum_for(:ai_status).with_values(pending: 0, processing: 1, completed: 2, failed: 3).with_prefix(:ai) }
  describe "#variable_definitions" do
    it "returns schema when present" do
      t.variables_schema = [ { "key"=>"n", "label"=>"N" } ]
      expect(t.variable_definitions).to eq([ { "key"=>"n", "label"=>"N" } ])
    end
    it "returns [] when nil" do
      t.variables_schema = nil
      expect(t.variable_definitions).to eq([])
    end
  end
  describe "#extract_used_variables" do
    it "finds Liquid vars" do
      t.html_content = "<p>{{ a }}, {{ b }}</p>"
      expect(t.extract_used_variables).to contain_exactly("a", "b")
    end
    it "returns [] when blank" do
      t.html_content = ""
      expect(t.extract_used_variables).to eq([])
    end
  end
  describe "#rendered_html" do
    it "replaces variables" do
      t.html_content = "<p>{{ n }}</p>"
      expect(t.rendered_html("n"=>"X")).to eq("<p>X</p>")
    end
    it "leaves missing blank" do
      t.html_content = "<p>{{ n }}</p>"
      expect(t.rendered_html({})).to eq("<p></p>")
    end
    it "returns '' when blank" do
      t.html_content = ""
      expect(t.rendered_html("n"=>"X")).to eq("")
    end
  end
end

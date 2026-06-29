require "rails_helper"

RSpec.describe EmailTemplate, type: :model do
  subject(:t) { build(:email_template) }

  it { is_expected.to belong_to(:workspace) }
  it { is_expected.to have_many(:email_template_documents).dependent(:destroy) }
  it { is_expected.to have_many(:document_templates).through(:email_template_documents) }
  it { is_expected.to validate_presence_of(:name) }
  it { is_expected.to define_enum_for(:ai_status).with_values(pending: 0, processing: 1, completed: 2, failed: 3).with_prefix(:ai) }

  describe ".usable" do
    it "includes templates with a body and excludes empty ones" do
      with_body = create(:email_template, body_html: "<p>hi</p>")
      create(:email_template, body_html: "")
      expect(EmailTemplate.usable).to contain_exactly(with_body)
    end
  end

  describe "#variable_definitions" do
    it "returns the schema when present" do
      t.variables_schema = [ { "key" => "n", "label" => "N" } ]
      expect(t.variable_definitions).to eq([ { "key" => "n", "label" => "N" } ])
    end

    it "returns [] when nil" do
      t.variables_schema = nil
      expect(t.variable_definitions).to eq([])
    end
  end

  describe "#extract_used_variables" do
    it "finds Liquid vars across subject and body" do
      t.subject = "Hello {{ a }}"
      t.body_html = "<p>{{ b }} and {{ a }}</p>"
      expect(t.extract_used_variables).to contain_exactly("a", "b")
    end

    it "returns [] when both are blank" do
      t.subject = ""
      t.body_html = ""
      expect(t.extract_used_variables).to eq([])
    end
  end

  describe "#rendered_subject / #rendered_body" do
    it "renders Liquid variables" do
      t.subject = "Hi {{ n }}"
      t.body_html = "<p>{{ n }}</p>"
      expect(t.rendered_subject("n" => "Ada")).to eq("Hi Ada")
      expect(t.rendered_body("n" => "Ada")).to eq("<p>Ada</p>")
    end

    it "leaves missing variables blank" do
      t.body_html = "<p>{{ n }}</p>"
      expect(t.rendered_body({})).to eq("<p></p>")
    end

    it "returns '' for blank templates" do
      t.subject = ""
      expect(t.rendered_subject("n" => "X")).to eq("")
    end
  end
end

require "rails_helper"
RSpec.describe DocumentTemplates::Filler do
  let(:h) { "<p>{{ a }}, {{ b }}.</p>" }
  it "replaces vars" do
    expect(described_class.call(h,{"a"=>"X","b"=>"Y"})).to eq("<p>X, Y.</p>")
  end
  it "leaves missing blank" do
    expect(described_class.call(h,{"a"=>"X"})).to eq("<p>X, .</p>")
  end
  it "returns '' for blank" do
    expect(described_class.call("",{})).to eq("")
  end
  it "handles nil" do
    expect(described_class.call(h,nil)).to eq("<p>, .</p>")
  end
end

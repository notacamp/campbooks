require "rails_helper"

RSpec.describe Identity do
  it "is valid with the factory defaults" do
    expect(build(:identity)).to be_valid
  end

  it "requires a provider in the allowed set" do
    expect(build(:identity, provider: nil)).not_to be_valid
    expect(build(:identity, provider: "facebook")).not_to be_valid
    expect(build(:identity, provider: "zoho")).to be_valid
  end

  it "requires a uid" do
    expect(build(:identity, uid: nil)).not_to be_valid
  end

  it "enforces (provider, uid) uniqueness — same uid is allowed across providers" do
    create(:identity, provider: "google", uid: "dup")
    expect(build(:identity, provider: "google", uid: "dup")).not_to be_valid
    expect(build(:identity, provider: "microsoft", uid: "dup")).to be_valid
  end

  it "humanizes the provider for display" do
    expect(build(:identity, provider: "microsoft").provider_label).to eq("Microsoft")
  end
end

require "rails_helper"

RSpec.describe Connection do
  it "requires name and base_url" do
    connection = described_class.new(workspace: build(:workspace), auth_type: "none")
    expect(connection).not_to be_valid
    expect(connection.errors.attribute_names).to include(:name, :base_url)
  end

  it "rejects a non-http base_url" do
    connection = build(:connection, base_url: "ftp://nope.example.com")
    expect(connection).not_to be_valid
    expect(connection.errors[:base_url]).to be_present
  end

  it "strips a trailing slash from base_url" do
    connection = create(:connection, base_url: "https://api.example.com/")
    expect(connection.base_url).to eq("https://api.example.com")
  end

  it "encrypts auth_secret at rest" do
    connection = create(:connection, :bearer)

    expect(connection.reload.auth_secret).to eq("tok_secret")
    raw = ActiveRecord::Base.connection.select_value(
      "SELECT auth_secret FROM connections WHERE id = #{connection.id}"
    )
    expect(raw).to be_present
    expect(raw).not_to include("tok_secret")
  end

  describe "#auth_headers" do
    it "is empty for none" do
      expect(build(:connection, auth_type: "none").auth_headers).to eq({})
    end

    it "builds a bearer header" do
      expect(build(:connection, :bearer).auth_headers).to eq("Authorization" => "Bearer tok_secret")
    end

    it "builds a custom header" do
      expect(build(:connection, :header).auth_headers).to eq("X-Api-Key" => "key_secret")
    end

    it "builds a basic header" do
      header = build(:connection, :basic).auth_headers["Authorization"]
      expect(header).to eq("Basic #{Base64.strict_encode64("user:pass")}")
    end
  end

  it "requires auth_header_name when auth_type is header" do
    connection = build(:connection, auth_type: "header", auth_header_name: nil, auth_secret: "x")
    expect(connection).not_to be_valid
    expect(connection.errors[:auth_header_name]).to be_present
  end
end

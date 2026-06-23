require "rails_helper"

RSpec.describe Push::ApnsSender do
  let(:user) { create(:user) }
  let(:device) { create(:device, user: user, platform: :ios, token: "abc") }
  let(:connection) { instance_double(Apnotic::Connection) }
  subject(:sender) { described_class.new(connection: connection) }

  before { allow(Push).to receive(:apns_bundle_id).and_return("com.notacamp.campbooks") }

  def apns_response(status, body = {})
    instance_double(Apnotic::Response, status: status.to_s, body: body)
  end

  it "returns :ok on 200" do
    allow(connection).to receive(:push).and_return(apns_response(200))
    expect(sender.deliver(device, title: "Hi", body: "There", url: "/x")).to eq(:ok)
  end

  it "returns :invalid on 410 (device unregistered)" do
    allow(connection).to receive(:push).and_return(apns_response(410))
    expect(sender.deliver(device, title: "Hi")).to eq(:invalid)
  end

  it "returns :invalid on 400 BadDeviceToken" do
    allow(connection).to receive(:push).and_return(apns_response(400, { "reason" => "BadDeviceToken" }))
    expect(sender.deliver(device, title: "Hi")).to eq(:invalid)
  end

  it "returns :error on 400 with an unrelated reason" do
    allow(connection).to receive(:push).and_return(apns_response(400, { "reason" => "PayloadTooLarge" }))
    expect(sender.deliver(device, title: "Hi")).to eq(:error)
  end

  it "returns :error on a timeout (nil response)" do
    allow(connection).to receive(:push).and_return(nil)
    expect(sender.deliver(device, title: "Hi")).to eq(:error)
  end

  it "addresses the notification to the device token + app topic" do
    captured = nil
    allow(connection).to receive(:push) { |n| captured = n; apns_response(200) }

    sender.deliver(device, title: "T", body: "B", url: "/u")

    expect(captured.token).to eq("abc")
    expect(captured.topic).to eq("com.notacamp.campbooks")
    expect(captured.alert).to eq(title: "T", body: "B")
    expect(captured.custom_payload).to eq(url: "/u")
  end
end

require "rails_helper"

RSpec.describe Push::FcmSender do
  let(:user) { create(:user) }
  let(:device) { create(:device, user: user, platform: :android, token: "fcm-tok") }
  let(:connection) { instance_double(Faraday::Connection) }
  subject(:sender) { described_class.new(connection: connection, access_token: "test-token") }

  before { allow(Push).to receive(:fcm_project_id).and_return("proj-1") }

  def fcm_response(status, body = "{}")
    instance_double(Faraday::Response, status: status, body: body)
  end

  it "returns :ok on 200 and targets the v1 send endpoint" do
    allow(connection).to receive(:post)
      .and_yield(double("req", headers: {}, "body=": nil))
      .and_return(fcm_response(200))

    expect(sender.deliver(device, title: "Hi", body: "There", url: "/x")).to eq(:ok)
    expect(connection).to have_received(:post).with("/v1/projects/proj-1/messages:send")
  end

  it "returns :invalid on 404 (unregistered)" do
    allow(connection).to receive(:post).and_return(fcm_response(404, '{"error":{"status":"NOT_FOUND"}}'))
    expect(sender.deliver(device, title: "Hi")).to eq(:invalid)
  end

  it "returns :invalid when error details carry UNREGISTERED" do
    body = '{"error":{"status":"INVALID_ARGUMENT","details":[{"errorCode":"UNREGISTERED"}]}}'
    allow(connection).to receive(:post).and_return(fcm_response(400, body))
    expect(sender.deliver(device, title: "Hi")).to eq(:invalid)
  end

  it "returns :error on other failures" do
    allow(connection).to receive(:post).and_return(fcm_response(403, '{"error":{"status":"PERMISSION_DENIED"}}'))
    expect(sender.deliver(device, title: "Hi")).to eq(:error)
  end
end

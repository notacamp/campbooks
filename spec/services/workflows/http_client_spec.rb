require "rails_helper"

RSpec.describe Workflows::HttpClient, type: :service do
  let(:connection) { instance_double(Faraday::Connection) }

  def response(status:, body:, headers: {}, success:)
    instance_double(Faraday::Response, status: status, body: body, headers: headers, success?: success)
  end

  describe ".call" do
    it "returns a normalized success result for a 2xx response" do
      allow(connection).to receive(:run_request)
        .with(:post, "https://api.example.com/hook", '{"a":1}', { "Content-Type" => "application/json" })
        .and_return(response(status: 200, body: "ok", success: true))

      result = described_class.call(
        method: "POST",
        url: "https://api.example.com/hook",
        headers: { "Content-Type" => "application/json" },
        body: '{"a":1}',
        connection: connection
      )

      expect(result).to include(ok: true, status: 200, body: "ok", error: nil)
    end

    it "marks non-2xx responses as not ok but still captures the body" do
      allow(connection).to receive(:run_request)
        .and_return(response(status: 422, body: "bad payload", success: false))

      result = described_class.call(method: "POST", url: "https://api.example.com/x", connection: connection)

      expect(result).to include(ok: false, status: 422, body: "bad payload")
    end

    it "returns a failure result when the URL is blocked (never makes the request)" do
      expect(connection).not_to receive(:run_request)

      result = described_class.call(method: "POST", url: "http://169.254.169.254/", connection: connection)

      expect(result[:ok]).to be(false)
      expect(result[:status]).to eq(0)
      expect(result[:error]).to match(/internal host/)
    end

    it "returns a failure result on timeout" do
      allow(connection).to receive(:run_request).and_raise(Faraday::TimeoutError)

      result = described_class.call(method: "GET", url: "https://api.example.com", connection: connection)

      expect(result[:ok]).to be(false)
      expect(result[:error]).to match(/timed out/)
    end

    it "returns a failure result on connection failure" do
      allow(connection).to receive(:run_request).and_raise(Faraday::ConnectionFailed.new("getaddrinfo"))

      result = described_class.call(method: "GET", url: "https://api.example.com", connection: connection)

      expect(result[:ok]).to be(false)
      expect(result[:error]).to match(/Connection failed/)
    end
  end
end

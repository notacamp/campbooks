require "rails_helper"

RSpec.describe "Email OTP login challenge", type: :request do
  let(:user) { create(:user) }

  before { user.update!(email_otp_enabled_at: Time.current) }

  def login
    post session_path, params: { email_address: user.email_address, password: "password123" }
  end

  # Trigger send_email_code and capture the plaintext code handed to the mailer.
  def request_code
    code = nil
    allow(VerificationMailer).to receive(:verify).and_wrap_original do |orig, **kwargs|
      code = kwargs[:code]
      orig.call(**kwargs)
    end
    post send_email_code_session_challenge_path
    code
  end

  it "emails a code and signs in when it is entered" do
    login
    code = request_code
    expect(code).to match(/\A\d{6}\z/)

    post session_challenge_path, params: { method: "email_otp", code: code }
    expect(response).to redirect_to(root_url)
  end

  it "rejects a wrong code" do
    login
    request_code

    post session_challenge_path, params: { method: "email_otp", code: "000000" }
    expect(response).to have_http_status(:unprocessable_entity)
  end

  it "locks the challenge after too many wrong attempts" do
    login
    code = request_code

    MfaEmailChallenge::MAX_ATTEMPTS.times do
      post session_challenge_path, params: { method: "email_otp", code: "000000" }
    end

    # Even the right code is refused now — a fresh code must be requested.
    post session_challenge_path, params: { method: "email_otp", code: code }
    expect(response).to have_http_status(:unprocessable_entity)
  end
end

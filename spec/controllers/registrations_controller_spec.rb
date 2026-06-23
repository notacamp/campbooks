require "rails_helper"

RSpec.describe RegistrationsController, type: :controller do
  render_views

  after do
    Current.reset
  end

  describe "PATCH complete" do
    before do
      session[:registration_state] = {
        email: "invited@example.com",
        name: "Invited User",
        code: "123456",
        code_sent_at: Time.current.iso8601,
        verified: true
      }
    end

    it "creates user in invitation org when valid token is in session" do
      org = create(:workspace, name: "Target Org")
      inviter = create(:user, workspace: org)
      invitation = create(:invitation, workspace: org, invited_by: inviter,
                          email: "invited@example.com")
      session[:invitation_token] = invitation.token

      expect {
        patch :complete, params: { password: "password123" }
      }.to change(User, :count).by(1)

      new_user = User.find_by(email_address: "invited@example.com")
      expect(new_user.workspace).to eq(org)
      expect(new_user.terms_accepted_at).to be_present
      expect(invitation.reload).to be_accepted
      expect(session[:invitation_token]).to be_nil
      expect(response).to redirect_to(root_path)
    end

    it "creates new org when no invitation token in session" do
      expect {
        patch :complete, params: { password: "password123" }
      }.to change(User, :count).by(1).and change(Workspace, :count).by(1)

      expect(response).to redirect_to(onboarding_path(step: :workspace))
    end

    it "creates new org when invitation token email does not match" do
      org = create(:workspace)
      inviter = create(:user, workspace: org)
      create(:invitation, workspace: org, invited_by: inviter, email: "different@example.com")
      session[:invitation_token] = Invitation.last.token

      expect {
        patch :complete, params: { password: "password123" }
      }.to change(Workspace, :count).by(1)
    end
  end

  describe "POST create" do
    render_views false

    around do |example|
      original_mode = Rails.application.config.signup_mode
      example.run
      Rails.application.config.signup_mode = original_mode
    end

    def set_signup_mode(mode)
      Rails.application.config.signup_mode = mode
    end

    context "when signup_mode is :open" do
      before { set_signup_mode(:open) }

      it "sends a verification code and advances to the verify step" do
        post :create, params: { name: "New User", email_address: "new@example.com", terms_accepted: "1" }

        expect(response).to redirect_to(verify_registration_path)
        expect(session[:registration_state]["email"]).to eq("new@example.com")
      end
    end

    it "refuses to create an account without terms + privacy consent" do
      set_signup_mode(:open)
      post :create, params: { name: "New User", email_address: "noconsent@example.com" }

      expect(response).to have_http_status(:unprocessable_entity)
      expect(session[:registration_state]).to be_nil
    end

    context "when signup_mode is :beta_code" do
      before { set_signup_mode(:beta_code) }

      it "rejects an unknown invite code without starting the flow" do
        # Campbooks::Input dasherizes the field name, so the real form posts "beta-code".
        post :create, params: { name: "New User", email_address: "new@example.com", "beta-code" => "NOPE-9999", terms_accepted: "1" }

        expect(response).to have_http_status(:unprocessable_entity)
        expect(session[:registration_state]).to be_nil
      end

      it "advances with a valid, unredeemed invite code and remembers it" do
        code = BetaCode.create!

        post :create, params: { name: "New User", email_address: "new@example.com", "beta-code" => code.code, terms_accepted: "1" }

        expect(response).to redirect_to(verify_registration_path)
        expect(session[:registration_state]["beta_code"]).to eq(code.code)
      end

      it "rejects an already-redeemed invite code" do
        code = BetaCode.create!
        code.redeem!(create(:user))

        post :create, params: { name: "New User", email_address: "new@example.com", "beta-code" => code.code, terms_accepted: "1" }

        expect(response).to have_http_status(:unprocessable_entity)
      end
    end

    context "when signup_mode is :invite_only" do
      before { set_signup_mode(:invite_only) }

      it "blocks public signup without an invitation" do
        post :create, params: { name: "New User", email_address: "new@example.com", terms_accepted: "1" }

        expect(response).to have_http_status(:unprocessable_entity)
        expect(session[:registration_state]).to be_nil
      end

      it "lets an invited email through the gate" do
        org = create(:workspace)
        inviter = create(:user, workspace: org)
        invitation = create(:invitation, workspace: org, invited_by: inviter, email: "invited@example.com")
        session[:invitation_token] = invitation.token

        post :create, params: { name: "Invited", email_address: "invited@example.com", terms_accepted: "1" }

        expect(response).to redirect_to(verify_registration_path)
      end
    end
  end

  describe "PATCH complete redeeming beta codes" do
    render_views false

    around do |example|
      original = Rails.application.config.signup_mode
      Rails.application.config.signup_mode = :beta_code
      example.run
      Rails.application.config.signup_mode = original
    end

    it "redeems the invite code and creates the account" do
      code = BetaCode.create!
      session[:registration_state] = {
        email: "beta@example.com", name: "Beta User",
        code: "123456", code_sent_at: Time.current.iso8601,
        verified: true, beta_code: code.code
      }

      expect {
        patch :complete, params: { password: "password123" }
      }.to change(User, :count).by(1)

      expect(code.reload).to be_redeemed
      expect(code.redeemed_by).to eq(User.find_by(email_address: "beta@example.com"))
      expect(response).to redirect_to(onboarding_path(step: :workspace))
    end

    it "refuses to create an account when the stored code is gone" do
      session[:registration_state] = {
        email: "beta@example.com", name: "Beta User",
        code: "123456", code_sent_at: Time.current.iso8601,
        verified: true, beta_code: "MISSING-CODE"
      }

      expect {
        patch :complete, params: { password: "password123" }
      }.not_to change(User, :count)

      expect(response).to have_http_status(:unprocessable_entity)
    end
  end
end

require "rails_helper"

RSpec.describe "Settings::Account", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user) { create(:user, workspace: workspace, password: "password123", password_confirmation: "password123") }

  describe "GET /settings/account/delete" do
    it "redirects unauthenticated users to sign-in" do
      get delete_settings_account_path
      expect(response).to redirect_to(new_session_path)
    end

    it "renders the delete confirmation page for authenticated users" do
      sign_in(user)
      get delete_settings_account_path
      expect(response).to have_http_status(:ok)
    end
  end

  describe "DELETE /settings/account" do
    context "with wrong password" do
      it "does not enqueue deletion and returns 422" do
        sign_in(user)
        expect {
          delete settings_account_path, params: {
            current_password: "wrong_password",
            confirm_email: user.email_address
          }
        }.not_to have_enqueued_job(AccountDeletionJob)
        expect(response).to have_http_status(:unprocessable_entity)
      end
    end

    context "with wrong email confirmation" do
      it "does not enqueue deletion and returns 422" do
        sign_in(user)
        expect {
          delete settings_account_path, params: {
            current_password: "password123",
            confirm_email: "wrong@example.com"
          }
        }.not_to have_enqueued_job(AccountDeletionJob)
        expect(response).to have_http_status(:unprocessable_entity)
      end
    end

    context "with correct credentials" do
      it "sets deletion_requested_at on the user" do
        sign_in(user)
        before = Time.current
        delete settings_account_path, params: {
          current_password: "password123",
          confirm_email: user.email_address
        }
        expect(user.reload.deletion_requested_at).to be >= before
      end

      it "enqueues AccountDeletionJob" do
        sign_in(user)
        expect {
          delete settings_account_path, params: {
            current_password: "password123",
            confirm_email: user.email_address
          }
        }.to have_enqueued_job(AccountDeletionJob).with(user.id)
      end

      it "terminates the session and redirects to sign-in" do
        sign_in(user)
        delete settings_account_path, params: {
          current_password: "password123",
          confirm_email: user.email_address
        }
        expect(response).to redirect_to(new_session_path)

        # Session should be gone — a subsequent authenticated request redirects
        get settings_account_path
        expect(response).to redirect_to(new_session_path)
      end
    end
  end

  describe "PATCH /settings/account/writing_style" do
    it "saves the writing style without requiring a password" do
      sign_in(user)
      patch writing_style_settings_account_path, params: { writing_style: "Breezy, signs off as Sam." }
      expect(response).to redirect_to(settings_account_path)
      expect(user.reload.writing_style).to eq("Breezy, signs off as Sam.")
      expect(user.writing_style_updated_at).to be_present
    end

    it "requires authentication" do
      patch writing_style_settings_account_path, params: { writing_style: "x" }
      expect(response).to redirect_to(new_session_path)
    end
  end

  describe "POST /settings/account/analyze_writing_style" do
    it "enqueues the style profiler for the current user" do
      sign_in(user)
      expect {
        post analyze_writing_style_settings_account_path
      }.to have_enqueued_job(WritingStyleProfileJob).with(user.id)
      expect(response).to redirect_to(settings_account_path)
    end
  end

  describe "sign-in blocked while deletion pending" do
    it "refuses sign-in when deletion_requested_at is set" do
      user.update!(deletion_requested_at: Time.current)
      post session_path, params: { email_address: user.email_address, password: "password123" }
      expect(response).to redirect_to(new_session_path)
      # Subsequent authenticated request still redirects to sign-in
      get settings_account_path
      expect(response).to redirect_to(new_session_path)
    end
  end
end

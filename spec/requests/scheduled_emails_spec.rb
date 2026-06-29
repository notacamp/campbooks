require "rails_helper"

# Scheduled emails are a paid feature: creating/editing one needs the
# email_scheduling entitlement (Free is blocked), while viewing and cancelling
# stay open so a downgraded workspace can still stop schedules it created.
RSpec.describe "Scheduled emails", type: :request do
  let(:user) { create(:user) }
  let!(:account) do
    create(:email_account, workspace: user.workspace).tap do |acct|
      create(:email_account_user, :collaborator, user: user, email_account: acct)
    end
  end

  before { sign_in(user) }

  def valid_params
    {
      email_account_id: account.id,
      to_address: "client@example.com",
      subject: "Hello",
      body: "<p>Body</p>",
      scheduled_at: 2.hours.from_now.strftime("%Y-%m-%dT%H:%M")
    }
  end

  describe "the entitlement gate" do
    context "on the Free plan" do
      it "blocks the new form" do
        get new_scheduled_email_path
        expect(response).to have_http_status(:redirect)
      end

      it "blocks create and persists nothing" do
        expect { post scheduled_emails_path, params: { scheduled_email: valid_params } }
          .not_to change(ScheduledEmail, :count)
        expect(response).to have_http_status(:redirect)
      end

      it "still serves the index (so downgraded workspaces can manage existing schedules)" do
        get scheduled_emails_path
        expect(response).to have_http_status(:ok)
      end
    end

    context "on the Pro plan" do
      before { user.workspace.update!(plan: "pro") }

      it "renders the new form" do
        get new_scheduled_email_path
        expect(response).to have_http_status(:ok)
      end

      it "creates a one-time schedule and stamps next_occurrence_at" do
        expect { post scheduled_emails_path, params: { scheduled_email: valid_params } }
          .to change(ScheduledEmail, :count).by(1)

        se = ScheduledEmail.last
        expect(se.workspace).to eq(user.workspace)
        expect(se.created_by).to eq(user)
        expect(se.next_occurrence_at).to be_present
        expect(response).to redirect_to(scheduled_email_path(se))
      end

      it "refuses an account the user can't send from and persists nothing" do
        foreign = create(:email_account, workspace: create(:workspace))

        expect { post scheduled_emails_path, params: { scheduled_email: valid_params.merge(email_account_id: foreign.id) } }
          .not_to change(ScheduledEmail, :count)
        expect(response).to have_http_status(:unprocessable_entity)
      end
    end
  end

  describe "cancelling" do
    let!(:scheduled) do
      create(:scheduled_email, workspace: user.workspace, created_by: user, email_account: account)
    end

    it "marks the schedule cancelled" do
      delete scheduled_email_path(scheduled)
      expect(scheduled.reload).to be_cancelled
      expect(response).to redirect_to(scheduled_emails_path)
    end

    it "stays available after a downgrade to Free" do
      user.workspace.update!(plan: "free")
      delete scheduled_email_path(scheduled)
      expect(scheduled.reload).to be_cancelled
    end
  end

  describe "workspace isolation" do
    it "404s a schedule from another workspace" do
      other = create(:scheduled_email)
      get scheduled_email_path(other)
      expect(response).to have_http_status(:not_found)
    end
  end
end

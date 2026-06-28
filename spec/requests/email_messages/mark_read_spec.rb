require "rails_helper"

# Opening a thread must do two things at once:
#   • mark its messages `read` — the inbox unread dot/bold, the "unread" filters,
#     and Scout's unread counts all read the `read` boolean; and
#   • stamp `viewed_at` — which clears the Mail nav attention dot
#     (Navigation::Attention#new_mail?).
# A regression once replaced the `read: true` write with a `viewed_at`-only write,
# leaving opened mail stuck "unread" across the inbox until the next provider sync.
# These specs lock both writes in.
RSpec.describe "Opening a thread marks it read and viewed", type: :request do
  let(:workspace) { create(:workspace) }
  let(:user)      { create(:user, workspace: workspace) }
  let(:account)   { create(:email_account, workspace: workspace) }
  let(:thread)    { EmailThread.create!(subject: "Project update", email_account: account) }

  before do
    create(:email_account_user, user: user, email_account: account, can_read: true)
    # Keep the show render offline and isolate the read/viewed_at writes from the
    # live-inbox broadcast (its own concern, covered elsewhere).
    allow_any_instance_of(EmailMessagesController)
      .to receive(:folder_mappings)
      .and_return({ name_to_ids: {}, id_to_name: {}, id_to_account: {} })
    allow(Emails::InboxBroadcaster).to receive(:replace)
    sign_in(user)
  end

  def open_thread(message)
    get email_message_path(message, folder_id: "all")
  end

  it "marks every unread message in the thread read and stamps viewed_at" do
    opened  = create(:email_message, email_account: account, email_thread: thread, read: false, viewed_at: nil)
    sibling = create(:email_message, email_account: account, email_thread: thread, read: false, viewed_at: nil)

    open_thread(opened)

    expect(opened.reload).to have_attributes(read: true)
    expect(sibling.reload).to have_attributes(read: true)
    expect(opened.viewed_at).to be_present
    expect(sibling.viewed_at).to be_present
  end

  it "stamps viewed_at on a message that synced in already-read but was never viewed" do
    # read=true (e.g. read on another device, pulled in by sync) but viewed_at is
    # still NULL, so it lights the Mail dot. Opening the thread must clear it too.
    msg = create(:email_message, email_account: account, email_thread: thread, read: true, viewed_at: nil)

    open_thread(msg)

    expect(msg.reload.viewed_at).to be_present
    expect(msg.read).to be true
  end

  it "enqueues the provider mark-read for the unread messages" do
    msg = create(:email_message, email_account: account, email_thread: thread, read: false, viewed_at: nil)

    expect { open_thread(msg) }.to have_enqueued_job(MarkReadJob)
  end

  it "does not enqueue a provider mark-read when nothing was unread" do
    msg = create(:email_message, email_account: account, email_thread: thread, read: true, viewed_at: Time.current)

    expect { open_thread(msg) }.not_to have_enqueued_job(MarkReadJob)
  end
end

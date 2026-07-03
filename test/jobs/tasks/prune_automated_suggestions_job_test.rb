# frozen_string_literal: true

require "test_helper"

module Tasks
  class PruneAutomatedSuggestionsJobTest < ActiveSupport::TestCase
    setup do
      @workspace = Workspace.create!(name: "Prune WS")
      @account = EmailAccount.create!(
        workspace: @workspace, email_address: "mailbox@example.com",
        provider: :google, refresh_token: "tok", active: true
      )
    end

    test "dismisses suggestions from vetoed mail, keeps human ones, never touches triaged tasks" do
      machine = suggestion_for(email(from: "no-reply@bot.example", category: "notifications"))
      human = suggestion_for(email(from: "ana@acme.test", category: "personal"))
      triaged = suggestion_for(email(from: "no-reply@bot.example", category: "notifications"))
      triaged.update!(status: :todo)
      orphan = Task.create!(
        workspace: @workspace, title: "Orphan", status: :suggested, priority: :normal,
        ai_suggested: true, source_type: "EmailMessage", source_id: SecureRandom.uuid
      )

      PruneAutomatedSuggestionsJob.perform_now

      assert machine.reload.cancelled?
      assert machine.archived?
      assert human.reload.suggested?
      assert triaged.reload.todo?
      assert orphan.reload.cancelled?
    end

    private

    def email(from:, category:)
      @account.email_messages.create!(
        provider_message_id: "m-#{SecureRandom.hex(4)}", provider_folder_id: "INBOX",
        from_address: from, to_address: "mailbox@example.com",
        subject: "Subject", body: "<p>Please do the thing.</p>", category: category,
        received_at: Time.current, read: false, has_attachment: false
      )
    end

    def suggestion_for(source)
      Task.create!(
        workspace: @workspace, title: "Task #{SecureRandom.hex(3)}", status: :suggested,
        priority: :normal, ai_suggested: true, source: source, confidence: 0.9
      )
    end
  end
end

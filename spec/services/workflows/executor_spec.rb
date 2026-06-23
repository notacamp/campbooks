require "rails_helper"

RSpec.describe Workflows::Executor, type: :service do
  let(:workspace) { create(:workspace) }
  let(:workflow) { create(:workflow, :webhook, workspace: workspace) }
  let(:context) { Workflows::WebhookContext.new(payload: { "event" => "invoice.paid", "amount" => 42 }) }

  def add_action(action_type, config)
    workflow.steps.create!(position: workflow.steps.count, step_type: "action", action_type: action_type, config: config)
  end

  def add_condition(config)
    workflow.steps.create!(position: workflow.steps.count, step_type: "condition", config: config)
  end

  describe "http_request action" do
    before do
      add_action("http_request", { http_method: "POST", url: "https://api.example.com/{{ payload.event }}", body: '{"amount":{{ payload.amount }}}' })
    end

    it "renders the request, performs it, and records the response" do
      expect(Workflows::HttpClient).to receive(:call).with(
        hash_including(method: "POST", url: "https://api.example.com/invoice.paid", body: '{"amount":42}')
      ).and_return(ok: true, status: 200, headers: {}, body: "ok", error: nil)

      execution = described_class.call(workflow, context)

      expect(execution.status).to eq("completed")
      step = execution.execution_steps.last
      expect(step.status).to eq("completed")
      expect(step.output_data.dig("response", "status")).to eq(200)
      expect(step.output_data.dig("request", "url")).to eq("https://api.example.com/invoice.paid")
    end

    it "fails the execution when the HTTP call is not ok" do
      allow(Workflows::HttpClient).to receive(:call).and_return(ok: false, status: 500, headers: {}, body: "boom", error: "HTTP 500")

      expect { described_class.call(workflow, context) }.to raise_error(/failed/)

      execution = workflow.executions.first
      expect(execution.status).to eq("failed")
      expect(execution.execution_steps.last.status).to eq("failed")
    end
  end

  describe "slack_message action" do
    before { add_action("slack_message", { webhook_url: "https://hooks.slack.com/services/T/B/X", text: "New {{ payload.event }}" }) }

    it "posts a Slack-shaped JSON body" do
      expect(Workflows::HttpClient).to receive(:call).with(
        hash_including(
          method: "POST",
          url: "https://hooks.slack.com/services/T/B/X",
          body: '{"text":"New invoice.paid"}'
        )
      ).and_return(ok: true, status: 200, headers: {}, body: "ok", error: nil)

      expect(described_class.call(workflow, context).status).to eq("completed")
    end
  end

  describe "discord_message action" do
    before { add_action("discord_message", { webhook_url: "https://discord.com/api/webhooks/1/x", content: "Hi {{ payload.event }}", username: "Bot" }) }

    it "posts a Discord-shaped JSON body including the username override" do
      expect(Workflows::HttpClient).to receive(:call).with(
        hash_including(body: '{"content":"Hi invoice.paid","username":"Bot"}')
      ).and_return(ok: true, status: 204, headers: {}, body: "", error: nil)

      expect(described_class.call(workflow, context).status).to eq("completed")
    end
  end

  describe "custom_action action" do
    let!(:connection) { create(:connection, :bearer, workspace: workspace, base_url: "https://api.example.com") }

    it "resolves the saved integration, merges its auth header, and joins base_url + path" do
      add_action("custom_action", {
        connection_id: connection.id, http_method: "POST",
        path: "/refunds/{{ payload.event }}", body: '{"amount":{{ payload.amount }}}'
      })

      captured = nil
      allow(Workflows::HttpClient).to receive(:call) do |**args|
        captured = args
        { ok: true, status: 200, headers: {}, body: "ok", error: nil }
      end

      expect(described_class.call(workflow, context).status).to eq("completed")
      expect(captured[:method]).to eq("POST")
      expect(captured[:url]).to eq("https://api.example.com/refunds/invoice.paid")
      expect(captured[:body]).to eq('{"amount":42}')
      expect(captured[:headers]).to include(
        "Authorization" => "Bearer tok_secret", "Content-Type" => "application/json"
      )
    end

    it "fails the execution when the integration cannot be found in the workspace" do
      add_action("custom_action", { connection_id: 0, path: "/x" })

      expect { described_class.call(workflow, context) }.to raise_error(/Integration not found/)
      expect(workflow.executions.first.status).to eq("failed")
    end
  end

  describe "email_action action (bridge into EmailActions)" do
    let(:owner) { create(:user, workspace: workspace) }
    let(:email_account) { create(:email_account, workspace: workspace) }
    let(:email_message) { create(:email_message, email_account: email_account, subject: "Invoice 42") }
    let(:email_workflow) { create(:workflow, workspace: workspace, created_by: owner) }
    let(:email_context) { Workflows::EmailContext.new(email_message) }

    it "runs the chosen tool on the triggering email, as the workflow owner, with rendered args" do
      email_workflow.steps.create!(position: 0, step_type: "action", action_type: "email_action",
        config: { email_tool: "add_tag", tag_name: "{{ email.subject }}" })
      allow(EmailActions).to receive(:run).and_return(success: true, tool: "add_tag", message: "Tagged", result: {})

      execution = described_class.call(email_workflow, email_context)

      expect(EmailActions).to have_received(:run).with(
        "add_tag",
        email_message: email_message,
        args: hash_including(tag_name: "Invoice 42"),
        user: owner
      )
      expect(execution.status).to eq("completed")
    end

    it "fails the step when EmailActions denies the action (e.g. no owner / no access)" do
      email_workflow.steps.create!(position: 0, step_type: "action", action_type: "email_action",
        config: { email_tool: "add_tag", tag_name: "x" })
      allow(EmailActions).to receive(:run).and_return(success: false, tool: "add_tag", message: "Access denied", result: nil)

      expect { described_class.call(email_workflow, email_context) }.to raise_error(/Email action/)
      expect(email_workflow.executions.first.status).to eq("failed")
    end

    it "fails closed on a non-email trigger (no email to act on)" do
      workflow.steps.create!(position: 0, step_type: "action", action_type: "email_action", config: { email_tool: "archive" })

      expect { described_class.call(workflow, context) }.to raise_error(/email-triggered/)
    end
  end

  describe "conditions" do
    it "halts before the action when the condition fails" do
      add_condition(field: "payload.event", operator: "equals", value: "refund.created")
      add_action("http_request", { url: "https://api.example.com/x" })

      expect(Workflows::HttpClient).not_to receive(:call)

      execution = described_class.call(workflow, context)
      expect(execution.status).to eq("completed")
      expect(execution.execution_steps.count).to eq(1) # condition only; action skipped
    end

    it "runs the action when the condition passes" do
      add_condition(field: "payload.event", operator: "equals", value: "invoice.paid")
      add_action("http_request", { url: "https://api.example.com/x" })

      allow(Workflows::HttpClient).to receive(:call).and_return(ok: true, status: 200, headers: {}, body: "ok", error: nil)

      execution = described_class.call(workflow, context)
      expect(execution.execution_steps.count).to eq(2)
    end
  end

  describe "send_email account scoping" do
    let(:account) { create(:email_account, workspace: workspace) }
    let(:foreign_account) { create(:email_account, workspace: create(:workspace)) }
    let(:config) { { email_account_id: account.id, to_template: "x@example.com", subject_template: "Hi", body_template: "Yo" } }

    it "refuses an account from outside the workflow's workspace" do
      add_action("send_email", config.merge(email_account_id: foreign_account.id))

      expect { described_class.call(workflow, context) }.to raise_error(/not found/i)
      expect(workflow.executions.first.status).to eq("failed")
    end

    it "sends from an account in the workflow's own workspace" do
      mail_client = instance_double(Zoho::MailClient, send_message: { "messageId" => "abc" })
      allow_any_instance_of(EmailAccount).to receive(:mail_client).and_return(mail_client)

      add_action("send_email", config)
      execution = described_class.call(workflow, context)

      expect(execution.status).to eq("completed")
      expect(mail_client).to have_received(:send_message)
    end
  end

  it "stores the trigger data on the execution" do
    add_action("http_request", { url: "https://api.example.com/x" })
    allow(Workflows::HttpClient).to receive(:call).and_return(ok: true, status: 200, headers: {}, body: "ok", error: nil)

    execution = described_class.call(workflow, context)
    expect(execution.trigger_data["type"]).to eq("webhook")
    expect(execution.trigger_data["payload"]).to eq("event" => "invoice.paid", "amount" => 42)
  end
end

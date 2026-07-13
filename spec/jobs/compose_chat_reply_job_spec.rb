require "rails_helper"

RSpec.describe ComposeChatReplyJob, type: :job do
  let(:user) { create(:user) }
  let(:thread) { create(:agent_thread, user: user, purpose: :compose_chat, title: "Compose") }
  let(:message) do
    create(:agent_message, agent_thread: thread, user: user, author_type: :user,
                           content: "Draft a quote request to Janis")
  end

  def service_returning(result)
    service = instance_double(Ai::ComposeChatService, reply_to: result)
    allow(Ai::ComposeChatService).to receive(:new).with(thread).and_return(service)
  end

  def broadcast_html
    captured = nil
    expect(Turbo::StreamsChannel).to have_received(:broadcast_replace_to) do |_stream, **kwargs|
      expect(kwargs[:target]).to eq("compose_messages_wrapper")
      captured = kwargs[:html]
    end
    captured
  end

  before { allow(Turbo::StreamsChannel).to receive(:broadcast_replace_to) }

  it "persists the reply and broadcasts auto_action scripts for the panel (#290)" do
    service_returning(
      reply: "Done — I set the subject and drafted the body.",
      auto_actions: [
        { "tool" => "set_subject", "args" => { "subject" => "Quote request" } },
        { "tool" => "set_body", "args" => { "body" => "<p>Hi Janis,</p>" } }
      ],
      suggested_actions: []
    )

    expect { described_class.perform_now(message.id) }
      .to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    html = broadcast_html
    expect(html).to include("__executeAutoAction__('set_subject'")
    expect(html).to include("__executeAutoAction__('set_body'")
  end

  it "drops auto_actions whose tool isn't in the known set (model output feeds a <script>)" do
    service_returning(
      reply: "ok",
      auto_actions: [
        { "tool" => "set_body", "args" => { "body" => "<p>x</p>" } },
        { "tool" => "evil'); alert(1); ('", "args" => {} },
        { "tool" => "forward_email", "args" => {} }
      ],
      suggested_actions: []
    )

    described_class.perform_now(message.id)

    html = broadcast_html
    expect(html).to include("__executeAutoAction__('set_body'")
    expect(html).not_to include("evil")
    expect(html).not_to include("forward_email")
  end

  it "still broadcasts the error reply when the AI fails, instead of crashing on nil" do
    service_returning(nil)

    expect { described_class.perform_now(message.id) }
      .to change { thread.agent_messages.where(author_type: :ai).count }.by(1)

    expect(thread.agent_messages.where(author_type: :ai).last.content)
      .to eq(I18n.t("jobs.compose_chat_reply.error"))
    expect(broadcast_html).to include("compose_messages_wrapper")
  end

  it "tolerates an auto_action with no args" do
    service_returning(
      reply: "ok",
      auto_actions: [ { "tool" => "send_email" } ],
      suggested_actions: []
    )

    described_class.perform_now(message.id)

    expect(broadcast_html).to include("__executeAutoAction__('send_email',{})")
  end
end

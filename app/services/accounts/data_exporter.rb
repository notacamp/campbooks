module Accounts
  # Assembles a machine-readable (JSON) copy of a user's personal data for the
  # GDPR right of access / portability (Art. 15 / 20). Scope = the user's own
  # Campbooks records (profile, sessions, AI conversations, signatures,
  # notifications, bug reports) plus metadata of the accounts they can access.
  # Bulk mailbox/document CONTENT originates in the user's own connected mailbox,
  # so it is summarised (which accounts are connected) rather than re-dumped here.
  class DataExporter
    def initialize(user)
      @user = user
      @workspace = user.workspace
    end

    def as_json(*)
      {
        meta: { exported_at: iso(Time.current), subject: @user.email_address, format: "campbooks-data-export/v1" },
        account: account_data,
        workspace: workspace_data,
        sessions: sessions_data,
        signatures: signatures_data,
        notifications: notifications_data,
        ai_conversations: ai_data,
        bug_reports: bug_reports_data,
        connected_email_accounts: email_accounts_data,
        connected_calendar_accounts: calendar_accounts_data,
        security_audit_log: audit_log_data
      }
    end

    def to_json(*)
      JSON.pretty_generate(as_json)
    end

    private

    def account_data
      { id: @user.id, name: @user.name, email_address: @user.email_address, locale: @user.locale,
        role: @user.role, created_at: iso(@user.created_at), terms_accepted_at: iso(@user.terms_accepted_at) }
    end

    def workspace_data
      @workspace && { id: @workspace.id, name: @workspace.name }
    end

    def sessions_data
      @user.sessions.order(:created_at).map do |s|
        { ip_address: s.ip_address, user_agent: s.user_agent, created_at: iso(s.created_at), last_active_at: iso(s.updated_at) }
      end
    end

    def signatures_data
      @user.signatures.order(:created_at).map { |s| { name: s.name, content: s.content, created_at: iso(s.created_at) } }
    end

    def notifications_data
      @user.notifications.order(created_at: :desc).limit(1000).map { |n| { body: n.body, created_at: iso(n.created_at) } }
    end

    def ai_data
      @user.agent_threads.includes(:agent_messages).order(:created_at).map do |thread|
        { title: thread.title, created_at: iso(thread.created_at),
          messages: thread.agent_messages.sort_by(&:created_at).map { |m| { author: m.author_type, content: m.content, created_at: iso(m.created_at) } } }
      end
    end

    def bug_reports_data
      @user.bug_reports.order(:created_at).map { |b| { description: b.description, page_url: b.page_url, created_at: iso(b.created_at) } }
    end

    def email_accounts_data
      @user.readable_email_accounts.map { |a| { email_address: a.email_address, provider: a.provider } }
    end

    def calendar_accounts_data
      @user.readable_calendar_accounts.map { |a| { email_address: a.email_address, provider: a.provider } }
    end

    def audit_log_data
      @user.audit_events.order(created_at: :desc).map do |e|
        { action: e.action, ip_address: e.ip_address, user_agent: e.user_agent,
          metadata: e.metadata, occurred_at: iso(e.created_at) }
      end
    end

    def iso(time)
      time&.utc&.iso8601
    end
  end
end

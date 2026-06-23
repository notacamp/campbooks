module InboxSettings
  # Inbox filtering panel: choose the whitelist/blacklist strategy and review the
  # blocked / starred / allowed sender lists (the place to undo a block). Renders
  # into the modal's `inbox_settings_panel` Turbo Frame.
  class FilteringController < BaseController
    def show
      load_panel
    end

    def update
      strategy = params[:inbox_filter_strategy].to_s
      if Workspace::INBOX_FILTER_STRATEGIES.include?(strategy)
        org = Current.workspace
        org.update(settings: org.settings.merge("inbox_filter_strategy" => strategy))
      end
      load_panel
      render :show
    end

    # Toggle a single sender's state from the panel, then re-render it.
    def set_sender
      contact = Current.workspace.contacts.find_by(id: params[:contact_id])
      apply_state(contact, params[:state].to_s) if contact
      load_panel
      render :show
    end

    private

    def apply_state(contact, state)
      case state
      when "unblock" then Contacts::Unblock.call(contact, user: Current.user)
      when "neutral" then contact.unblock!
      when "unstar"  then contact.unstar!
      when "block"   then Contacts::Block.call(contact, user: Current.user)
      when "allow"   then contact.allow!
      end
    end

    def load_panel
      @strategy = Current.workspace.inbox_filter_strategy
      contacts = Current.workspace.contacts
      @starred = contacts.starred.order(:name, :email).limit(50)
      @blocked = contacts.blocked.order(:name, :email).limit(50)
      @allowed = contacts.allowed.order(:name, :email).limit(50)
    end
  end
end

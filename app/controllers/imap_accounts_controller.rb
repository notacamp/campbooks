# frozen_string_literal: true

class ImapAccountsController < ApplicationController
  before_action :require_authentication
  before_action :require_imap_enabled

  def new
    @email_account = EmailAccount.new(
      imap_port: 993,
      imap_security: "ssl",
      smtp_port: 587,
      smtp_security: "starttls"
    )
  end

  def create
    normalized_email = imap_params[:email_address].to_s.strip.downcase
    existing = EmailAccount.find_by(email_address: normalized_email)

    if existing
      # Unlike the OAuth callbacks — where the provider itself proved the user
      # controls the mailbox — an IMAP "reconnect" only proves the credentials
      # work against WHATEVER HOST THE FORM SUPPLIED. Without a management check
      # anyone could point an existing address at their own server, pass
      # verification, and be granted an owner entry over the victim's already-
      # synced mail. So: reconnect is for managers of that account only; every
      # other collision gets the same refusal, leaking nothing about the account.
      if existing.imap? && existing.managed_by?(Current.user)
        # Reconnect — reuse its slot, bypass the plan cap.
        @email_account = existing
        @email_account.assign_attributes(imap_params)
        @email_account.email_address = normalized_email
        @email_account.active = true
        @email_account.backfill_since = backfill_since
      else
        @email_account = EmailAccount.new(imap_params)
        @email_account.assign_attributes(provider: :imap)
        flash.now[:error] = t(".already_connected_other_provider")
        return render :new, status: :unprocessable_entity
      end
    else
      # Brand-new mailbox — check the plan cap first.
      unless current_entitlements.allow?(:email_accounts) == :ok
        back = session[:onboarding_return_to] || email_messages_path(inbox_settings: "accounts")
        redirect_to(back, alert: t("entitlements.blocked.email_accounts_cap",
                                    limit: current_entitlements.limit(:email_accounts),
                                    plan:  current_entitlements.plan_name))
        return
      end

      @email_account = EmailAccount.new(
        provider:       :imap,
        workspace:      Current.workspace,
        backfill_since: backfill_since
      )
      @email_account.assign_attributes(imap_params)
      # Store the address in the same normalized form the collision lookup uses,
      # or "Demo@x" and "demo@x" would slip past uniqueness as distinct rows.
      @email_account.email_address = normalized_email
    end

    return render :new, status: :unprocessable_entity unless @email_account.valid?

    if (message = verification_error(@email_account))
      flash.now[:error] = message
      return render :new, status: :unprocessable_entity
    end

    @email_account.save!
    @email_account.email_account_users.find_or_create_by!(user: Current.user) do |entry|
      entry.owner      = true
      entry.can_read   = true
      entry.can_send   = true
      entry.can_manage = true
    end

    Events.publish(
      "email_account.connected",
      subject: @email_account,
      payload: { "email_address" => @email_account.email_address, "provider" => @email_account.provider }
    )
    EmailScanJob.perform_later(@email_account.id, "delta")

    destination = session.delete(:onboarding_return_to) || email_messages_path(inbox_settings: "accounts")
    redirect_to destination, success: t(".connected", email: @email_account.email_address)
  end

  def edit
    @email_account = require_manageable_imap_account(t(".not_permitted"))
  end

  def update
    @email_account = require_manageable_imap_account(t(".not_permitted"))
    return unless @email_account

    permitted = imap_params.to_h
    # Blank password means "keep the existing one" — only update when provided.
    permitted.delete("imap_password") if permitted["imap_password"].blank?
    @email_account.assign_attributes(permitted)
    @email_account.active = true

    return render :edit, status: :unprocessable_entity unless @email_account.valid?

    if (message = verification_error(@email_account))
      flash.now[:error] = message
      return render :edit, status: :unprocessable_entity
    end

    @email_account.save!
    redirect_to email_messages_path(inbox_settings: "accounts"), success: t(".updated")
  end

  private

  def require_imap_enabled
    head :not_found unless Features.imap?
  end

  # Looks up an IMAP account the current user can see and manage.
  # Returns the account on success; sets a 404/redirect and returns nil on failure.
  # deny_message must be evaluated by the *caller* so lazy t() keys resolve to the
  # right action scope (edit.not_permitted vs update.not_permitted).
  def require_manageable_imap_account(deny_message)
    account = Current.user.email_accounts.find_by(id: params[:id])

    unless account&.imap?
      head :not_found
      return nil
    end

    unless account.managed_by?(Current.user)
      redirect_to email_messages_path(inbox_settings: "accounts"), alert: deny_message
      return nil
    end

    account
  end

  # Live IMAP+SMTP credential check. Returns nil on success, or the flash
  # message for the failure. Absolute keys: create and update share these.
  def verification_error(account)
    Imap::MailClient.new(account).verify!
    nil
  rescue Imap::HostGuard::BlockedError
    t("imap_accounts.errors.blocked_host")
  rescue PermanentAuthError
    t("imap_accounts.errors.auth_rejected")
  rescue AuthenticationError, SocketError, IOError, OpenSSL::SSL::SSLError,
         Net::OpenTimeout, Net::ReadTimeout, Net::IMAP::Error,
         Errno::ECONNREFUSED, Errno::EHOSTUNREACH, Errno::ETIMEDOUT => e
    t("imap_accounts.errors.unreachable", error: e.class.name)
  end

  def backfill_since
    case params[:sync_history]
    when "30d" then 30.days.ago
    when "1y"  then 1.year.ago
    when "all" then nil
    else            90.days.ago
    end
  end

  def imap_params
    params.require(:email_account).permit(
      :email_address, :imap_password, :imap_username,
      :imap_host, :imap_port, :imap_security,
      :smtp_host, :smtp_port, :smtp_security
    )
  end
end

# frozen_string_literal: true

class ScheduledEmailsController < ApplicationController
  before_action :require_authentication
  before_action -> { require_entitlement!(:email_scheduling) }, except: [:index, :show]
  before_action :load_scheduled_email, only: %i[show edit update destroy]

  def index
    @scheduled_emails = ScheduledEmail.accessible_to(Current.user)
                                      .order(Arel.sql("COALESCE(next_occurrence_at, scheduled_at) ASC"))
                                      .includes(:email_account, :created_by)
  end

  def show
  end

  def new
    @scheduled_email = ScheduledEmail.new(
      email_account_id: params[:email_account_id],
      to_address: params[:to_address],
      subject: params[:subject],
      template_context: default_template_context
    )
    @email_accounts = Current.user.sendable_email_accounts.ordered
  end

  def create
    @scheduled_email = ScheduledEmail.new(scheduled_email_params)
    @scheduled_email.workspace = Current.workspace
    @scheduled_email.created_by = Current.user

    if @scheduled_email.save
      recalculate_next_occurrence
      redirect_to @scheduled_email, notice: t(".created")
    else
      @email_accounts = Current.user.sendable_email_accounts.ordered
      render :new, status: :unprocessable_entity
    end
  end

  def edit
    @email_accounts = Current.user.sendable_email_accounts.ordered
  end

  def update
    if @scheduled_email.update(scheduled_email_params)
      recalculate_next_occurrence
      redirect_to @scheduled_email, notice: t(".updated")
    else
      @email_accounts = Current.user.sendable_email_accounts.ordered
      render :edit, status: :unprocessable_entity
    end
  end

  def destroy
    @scheduled_email.update!(status: :cancelled)
    redirect_to scheduled_emails_path, notice: t(".cancelled")
  end

  private

  def load_scheduled_email
    @scheduled_email = ScheduledEmail.accessible_to(Current.user).find(params[:id])
  end

  def scheduled_email_params
    params.require(:scheduled_email).permit(
      :email_account_id, :to_address, :cc_address, :bcc_address,
      :subject, :body, :scheduled_at, :rrule,
      template_context: {}
    )
  end

  def recalculate_next_occurrence
    return unless @scheduled_email.pending?

    next_at = if @scheduled_email.rrule.present?
                ScheduleCalculator.next_occurrence(@scheduled_email.scheduled_at, @scheduled_email.rrule)
              else
                @scheduled_email.scheduled_at
              end
    @scheduled_email.update_columns(next_occurrence_at: next_at)
  end

  def default_template_context
    {
      "contact" => { "first_name" => "", "last_name" => "", "email" => "" },
      "date" => Date.current.iso8601,
      "workspace" => { "name" => Current.workspace&.name || "" }
    }
  end
end

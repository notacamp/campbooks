# frozen_string_literal: true

class ScheduledEmailsController < ApplicationController
  before_action :require_authentication
  # Creating/editing a schedule needs the entitlement, but viewing and
  # cancelling existing ones stays open so a downgraded workspace can still see
  # and stop schedules it created while subscribed.
  before_action -> { require_entitlement!(:email_scheduling) }, only: %i[new create edit update]
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
      scheduled_at: default_scheduled_at
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
      :subject, :body, :scheduled_at, :rrule
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

  # Sensible default for a brand-new schedule: the next half-hour, one hour out.
  def default_scheduled_at
    (Time.current + 1.hour).change(min: (Time.current.min / 30) * 30)
  end
end

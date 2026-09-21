# frozen_string_literal: true

module Api
  module App
    module Money
      # Loan CRUD and suggestion-dismiss, under /api/app/money/loans.
      # Every mutation returns the refreshed Money::Page so the SPA cache stays warm.
      class LoansController < Api::App::BaseController
        before_action :require_accounting_enabled
        before_action :require_accounting_entitlement
        before_action :set_loan, only: %i[show update destroy]

        # GET /api/app/money/loans
        def index
          loans = current_workspace.loans.active_loans.order(:created_at)
          render_data(loans.map { |l| LoanSerializer.new(l).as_json })
        end

        # POST /api/app/money/loans
        def create
          loan = current_workspace.loans.build(loan_params)
          loan.created_by = current_user

          if loan.save
            Loans::Schedule.build!(loan)
            Loans::Backfill.call(loan)
            render_data({ loan: LoanSerializer.new(loan).as_json, money: refreshed_money_page },
                        status: :created)
          else
            render_error("invalid", loan.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # GET /api/app/money/loans/:id
        def show
          Loans::Status.refresh!(@loan)
          instalments = @loan.instalments
                             .includes(bank_transaction: :reconciliation)
                             .ordered
          render_data({
            loan:        LoanSerializer.new(@loan).as_json,
            instalments: instalments.map { |i| LoanInstalmentSerializer.new(i).as_json }
          })
        end

        # PATCH /api/app/money/loans/:id
        def update
          if @loan.update(loan_params)
            if @loan.saved_changes.keys.intersect?(%w[term_months first_instalment_on instalment_cents])
              Loans::Schedule.rebuild!(@loan)
            end
            Loans::Backfill.call(@loan)
            render_data({ loan: LoanSerializer.new(@loan).as_json, money: refreshed_money_page })
          else
            render_error("invalid", @loan.errors.full_messages.to_sentence, status: :unprocessable_entity)
          end
        end

        # DELETE /api/app/money/loans/:id
        def destroy
          @loan.destroy!
          render_data({ money: refreshed_money_page })
        end

        # POST /api/app/money/loans/:id/dismiss
        # "Not a loan" — adds the suggestion key to the workspace's dismissed list.
        def dismiss
          key = params[:key].to_s.strip
          if key.present?
            settings  = current_workspace.settings.deep_dup
            dismissed = Array(settings["dismissed_loan_suggestions"])
            dismissed << key unless dismissed.include?(key)
            settings["dismissed_loan_suggestions"] = dismissed
            current_workspace.update!(settings: settings)
          end

          render_data({ money: refreshed_money_page })
        end

        private

        def require_accounting_enabled
          render_not_found unless Features.accounting?
        end

        def require_accounting_entitlement
          require_entitlement!(:accounting)
        end

        def set_loan
          @loan = current_workspace.loans.find(params[:id])
        end

        def loan_params
          raw = params.require(:loan).permit(
            :lender, :source_counterparty, :principal, :instalment,
            :first_instalment_on, :term_months, :rate_note, :notes, :change_acknowledged_at
          )
          raw[:principal_cents]  = parse_money_cents(raw.delete(:principal))  if raw.key?(:principal)
          raw[:instalment_cents] = parse_money_cents(raw.delete(:instalment)) if raw.key?(:instalment)
          raw
        end

        # Parses "46.800,00" / "46,800.00" / "46800" etc. into integer cents.
        def parse_money_cents(value)
          return 0 if value.blank?

          clean = value.to_s.gsub(/[€$£]|R\$/, "").gsub(/\s/, "")
          clean =
            if clean.match?(/\A\d{1,3}(\.\d{3})+(,\d{1,2})?\z/)
              clean.delete(".").tr(",", ".")
            elsif clean.match?(/\A\d{1,3}(,\d{3})+(\.\d{1,2})?\z/)
              clean.delete(",")
            elsif clean.include?(",") && !clean.include?(".")
              clean.tr(",", ".")
            else
              clean.delete(",")
            end

          (clean.to_f * 100).round
        end

        def refreshed_money_page
          Api::App::Money::PageSerializer.new(
            ::Money::Page.for(current_workspace, current_user, today: ::Date.current)
          ).as_json
        end
      end
    end
  end
end

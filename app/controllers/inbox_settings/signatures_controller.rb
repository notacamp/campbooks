module InboxSettings
  # Email signatures panel. Because the "default" flag is exclusive across the
  # set, every mutation re-renders the whole list rather than a single row.
  class SignaturesController < BaseController
    before_action :set_signature, only: [ :edit, :update, :destroy, :set_default ]

    def index
      @signatures = Current.user.signatures.ordered
    end

    def new
      @signature = Current.user.signatures.new
    end

    def create
      @signature = Current.user.signatures.new(signature_params)
      if @signature.save
        @signature.make_default! if @signature.is_default?
        @signatures = Current.user.signatures.ordered
        render :list_stream, locals: { message: "Signature created." }
      else
        render_form_errors
      end
    end

    def edit
    end

    def update
      if @signature.update(signature_params)
        @signature.make_default! if @signature.is_default?
        @signatures = Current.user.signatures.ordered
        render :list_stream, locals: { message: "Signature updated." }
      else
        render_form_errors
      end
    end

    def destroy
      @signature.destroy
      @signatures = Current.user.signatures.ordered
      render :list_stream, locals: { message: "Signature deleted." }
    end

    def set_default
      @signature.make_default!
      @signatures = Current.user.signatures.ordered
      render :list_stream, locals: { message: "Default signature updated." }
    end

    private

    def render_form_errors
      render turbo_stream: turbo_stream.update(
        "inbox_settings_signature_form",
        partial: "inbox_settings/signatures/form",
        locals: { signature: @signature }
      ), status: :unprocessable_entity
    end

    def set_signature
      @signature = Current.user.signatures.find(params[:id])
    end

    def signature_params
      permitted = params.require(:signature).permit(:name, :content, :is_default, email_account_ids: [])
      # Never let a signature be linked to an account the user doesn't own —
      # the has_many :through setter would happily create the join otherwise.
      if permitted[:email_account_ids]
        owned = Current.user.email_accounts.pluck(:id).map(&:to_s)
        permitted[:email_account_ids] = permitted[:email_account_ids].select { |id| owned.include?(id.to_s) }
      end
      permitted
    end
  end
end

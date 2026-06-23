class Admin::BetaCodesController < Admin::BaseController
  def index
    @beta_codes = BetaCode.chronological
  end

  def create
    codes = BetaCode.generate_batch(
      count: params[:count].presence || 10,
      label: params[:label],
      created_by: Current.user
    )
    redirect_to admin_beta_codes_path, success: t(".success", count: codes.size)
  end

  def destroy
    BetaCode.unredeemed.find(params[:id]).destroy
    redirect_to admin_beta_codes_path, success: t(".success")
  end
end

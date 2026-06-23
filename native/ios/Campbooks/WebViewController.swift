import HotwireNative
import UIKit

/// Web screen with **contextual** native chrome.
///
/// The native navigation bar is hidden on root (tab) screens — Home, Mail,
/// Calendar, Scout, Docs, Flows already have the in-app bottom nav and their own
/// in-page headers, so a native bar there is pure redundancy (and its generic
/// "Campbooks" title + back button were confusing). It's shown on *pushed*
/// detail screens (an open email, a document, a calendar event), where a back
/// button to return is genuinely useful. Modally-presented screens keep their
/// bar for the Done button.
///
/// "Root" is determined by stack position, not URL, so the multi-pane inbox
/// works automatically: the Mail tab is a root (no bar), but tapping a thread
/// pushes a detail screen (bar + back) — same URL space, different depth.
final class WebViewController: HotwireWebViewController {
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)

        let presentedModally = navigationController?.presentingViewController != nil
        let isRootScreen = navigationController?.viewControllers.first === self
        let hideBar = isRootScreen && !presentedModally

        navigationController?.setNavigationBarHidden(hideBar, animated: animated)
    }
}

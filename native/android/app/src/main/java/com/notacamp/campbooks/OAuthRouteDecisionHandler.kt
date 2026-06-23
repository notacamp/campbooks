package com.notacamp.campbooks

import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.net.toUri
import dev.hotwire.navigation.activities.HotwireActivity
import dev.hotwire.navigation.navigator.NavigatorConfiguration
import dev.hotwire.navigation.routing.Router

/**
 * Opens OAuth provider URLs (Google / Microsoft / Zoho) in a Chrome Custom Tab —
 * a real browser — instead of the embedded web view, which providers reject.
 *
 * The server bakes a signed, native-aware `state` into the authorize URL (see
 * the Rails Oauth::State + OauthNativeHandoff). When the dance finishes the
 * server redirects to campbooks://oauth?…, caught by MainActivity's intent-filter
 * (see AndroidManifest + MainActivity.handleOAuthDeepLink).
 */
class OAuthRouteDecisionHandler : Router.RouteDecisionHandler {
    override val name = "oauth-provider"

    override fun matches(
        location: String,
        configuration: NavigatorConfiguration
    ): Boolean {
        return location.toUri().host in Config.OAUTH_PROVIDER_HOSTS
    }

    override fun handle(
        location: String,
        configuration: NavigatorConfiguration,
        activity: HotwireActivity
    ): Router.Decision {
        CustomTabsIntent.Builder()
            .setShowTitle(true)
            .build()
            .launchUrl(activity, location.toUri())

        // We've taken over; never navigate the provider URL in-app.
        return Router.Decision.CANCEL
    }
}
